use anyhow::{Context, Result};
use chrono::{DateTime, ParseError, Utc};
use dotenv::dotenv;
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePool, Row};
use std::{env, fs, path::Path, sync::Arc};
use teloxide::RequestError;
use teloxide::{prelude::*, types::ParseMode, utils::command::BotCommands};
use thiserror::Error;
use tokio::time::{sleep, Duration};
use teloxide::types::ChatMemberKind;

#[derive(Error, Debug)]
enum BotError {
    #[error("A task with this name already exists")]
    TaskExists,

    #[error("Task not found")]
    TaskNotFound,

    #[error("Connection to X.AI service failed")]
    XaiServiceError(#[from] reqwest::Error),

    #[error("Database error")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Telegram API error")]
    TelegramError(#[from] RequestError),

    #[error("Invalid task parameters")]
    InvalidParameters,

    #[error("Date parsing error")]
    DateParseError(#[from] ParseError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    #[error("Permission denied")]
    PermissionDenied,
}

impl BotError {
    fn user_message(&self) -> String {
        let message = match self {
            BotError::TaskExists => {
                "❌ A task with this name already exists\\. Please choose a different name\\."
            }
            BotError::TaskNotFound => {
                "❌ Task not found\\. Use /list to see all available tasks\\."
            }
            BotError::XaiServiceError(_) => {
                "❌ Unable to reach X\\.AI service\\. Please try again later\\."
            }
            BotError::DatabaseError(e) => {
                if let sqlx::Error::Database(db_err) = e {
                    if db_err.code() == Some("1555".into())
                        || db_err.message().contains("UNIQUE constraint failed")
                    {
                        return "❌ A task with this name already exists\\. Please choose a different name\\.".to_string();
                    }
                }
                "❌ Unable to process your request\\. Please try again later\\."
            }
            BotError::TelegramError(_) => "❌ Unable to send message\\. Please try again later\\.",
            BotError::InvalidParameters => {
                "❌ Invalid parameters provided\\. Please check the command format and try again\\."
            }
            BotError::DateParseError(_) => {
                "❌ Error processing date information\\. Please try again later\\."
            }
            BotError::Other(_) => "❌ An unexpected error occurred\\. Please try again later\\.",
            BotError::PermissionDenied => {
                "❌ This command is restricted to the bot owner\\."
            },
        };
        message.to_string()
    }
}

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
enum Command {
    #[command(description = "Display this help message")]
    Help,
    #[command(description = "Show your Telegram ID")]
    MyId,
    #[command(description = "Create a new X.AI query task: /create <task_name> <interval_minutes> <question>")]
    Create(String),
    #[command(description = "List all tasks")]
    List,
    #[command(description = "Delete a task")]
    Delete(String),
    #[command(description = "Ask X.AI a one-time question")]
    Ask(String),
    #[command(description = "Get your usage statistics")]
    Stats,
    #[command(description = "Get overall bot usage statistics (bot owner only)")]
    BotStats,
}

struct AppState {
    pool: SqlitePool,
    http_client: Client,
    xai_token: String,
    owner_id: i64,  // Add this field
}

type State = Arc<AppState>;

async fn is_bot_creator(bot: &Bot, user_id: i64, _chat_id: i64, owner_id: i64) -> Result<bool, RequestError> {
    Ok(user_id == owner_id)
}


fn escape_non_formatting_chars(text: &str) -> String {
    let special_chars = [
        '[', ']', '(', ')', '~', '>', '#', '+', '-', '=', '|', 
        '{', '}', '.', '!', '\'', '"', '?', '$', '&', ',', ':', ';', '\\',
    ];
    
    let mut result = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if special_chars.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

fn escape_markdown_v2(text: &str) -> String {
    let special_chars = [
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', 
        '{', '}', '.', '!', '\'', '"', '?', '$', '&', ',', ':', ';', '\\',
    ];
    
    let mut result = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if special_chars.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

async fn initialize_database() -> Result<()> {
    let data_dir = Path::new("data");
    let db_path = data_dir.join("tasks.db");

    if !data_dir.exists() {
        fs::create_dir_all(data_dir).context("Failed to create data directory")?;
        log::info!("Created data directory");
    }

    if !db_path.exists() {
        fs::File::create(&db_path).context("Failed to create database file")?;
        log::info!("Created empty database file");
    }

    let database_url = format!("sqlite:{}", db_path.to_string_lossy());

    let pool = SqlitePool::connect(&database_url)
        .await
        .context("Failed to connect to SQLite database")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tasks (
            name TEXT PRIMARY KEY,
            description TEXT NOT NULL,
            interval INTEGER NOT NULL,
            last_run TEXT NOT NULL,
            chat_id INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .context("Failed to create tasks table")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS bot_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            chat_id INTEGER NOT NULL,
            user_id INTEGER,
            username TEXT,
            command TEXT NOT NULL,
            args TEXT,
            response TEXT,
            error TEXT,
            execution_time_ms INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .context("Failed to create logs table")?;

    log::info!("Database initialized successfully");
    Ok(())

}

async fn log_interaction(
    pool: &SqlitePool,
    chat_id: i64,
    user_id: Option<i64>,
    username: Option<String>,
    command: &str,
    args: Option<&str>,
    response: Option<&str>,
    error: Option<&str>,
    execution_time: Duration,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO bot_logs 
        (timestamp, chat_id, user_id, username, command, args, response, error, execution_time_ms)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(Utc::now().to_rfc3339())
    .bind(chat_id)
    .bind(user_id)
    .bind(username)
    .bind(command)
    .bind(args)
    .bind(response)
    .bind(error)
    .bind(execution_time.as_millis() as i64)
    .execute(pool)
    .await?;

    Ok(())
}

async fn get_user_stats(pool: &SqlitePool, user_id: i64) -> Result<Value, sqlx::Error> {
    let stats = sqlx::query(
        r#"
        SELECT 
            COUNT(*) as total_commands,
            COUNT(DISTINCT DATE(timestamp)) as active_days,
            AVG(execution_time_ms) as avg_execution_time,
            COUNT(CASE WHEN error IS NOT NULL THEN 1 END) as error_count
        FROM bot_logs 
        WHERE user_id = ?
        "#
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(json!({
        "total_commands": stats.get::<i64, _>("total_commands"),
        "active_days": stats.get::<i64, _>("active_days"),
        "avg_execution_time_ms": stats.get::<f64, _>("avg_execution_time"),
        "error_rate": (stats.get::<i64, _>("error_count") as f64 / stats.get::<i64, _>("total_commands") as f64 * 100.0)
    }))
}

async fn get_command_stats(pool: &SqlitePool) -> Result<Value, sqlx::Error> {
    let stats = sqlx::query(
        r#"
        SELECT 
            command,
            COUNT(*) as usage_count,
            AVG(execution_time_ms) as avg_execution_time,
            COUNT(CASE WHEN error IS NOT NULL THEN 1 END) as error_count
        FROM bot_logs 
        GROUP BY command
        ORDER BY usage_count DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(json!({
        "commands": stats.iter().map(|row| {
            json!({
                "command": row.get::<String, _>("command"),
                "usage_count": row.get::<i64, _>("usage_count"),
                "avg_execution_time_ms": row.get::<f64, _>("avg_execution_time"),
                "error_rate": (row.get::<i64, _>("error_count") as f64 / row.get::<i64, _>("usage_count") as f64 * 100.0)
            })
        }).collect::<Vec<_>>()
    }))
}

async fn parse_create_command(input: String) -> Option<(String, u64, String)> {
    let parts: Vec<&str> = input.splitn(3, ' ').collect();
    if parts.len() == 3 {
        let interval = parts[1].parse::<u64>().ok()?;
        Some((parts[0].to_string(), interval, parts[2].to_string()))
    } else {
        None
    }
}

fn format_xai_response(task_name: Option<&str>, question: &str, response: &str) -> String {
    match task_name {
        Some(name) => format!(
            "🤖 *Task Response*\n\n\
            📌 *Task:* {}\n\
            ❓ *Question:* `{}`\n\n\
            📝 *Answer:*\n\n{}",
            escape_markdown_v2(name),
            escape_markdown_v2(question),
            format_response_content(response)
        ),
        None => format!(
            "🤖 *X\\.AI Response*\n\n\
            ❓ *Question:* `{}`\n\n\
            📝 *Answer:*\n\n{}",
            escape_markdown_v2(question),
            format_response_content(response)
        ),
    }
}


fn format_response_content(content: &str) -> String {
    content
        .split("\n\n")
        .map(|paragraph| {
            // Handle lists
            if paragraph
                .lines()
                .any(|line| line.trim().starts_with('-') || line.trim().starts_with('*'))
            {
                paragraph
                    .lines()
                    .map(|line| {
                        if line.trim().starts_with('-') || line.trim().starts_with('*') {
                            let content = line
                                .trim()
                                .trim_start_matches(|c| c == '-' || c == '*')
                                .trim();
                            format!("• {}", process_markdown_formatting(content))
                        } else {
                            process_markdown_formatting(line)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                process_markdown_formatting(paragraph)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn process_markdown_formatting(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    let mut chars = text.chars().peekable();
    let mut in_format = None; // None, Some("bold"), Some("italic"), Some("code")
    let mut current_text = String::new();

    while let Some(c) = chars.next() {
        match c {
            '*' | '_' | '`' => {
                let format_type = match c {
                    '*' => "bold",
                    '_' => "italic",
                    '`' => "code",
                    _ => unreachable!(),
                };

                // Count consecutive formatting characters
                let mut count = 1;
                while chars.peek() == Some(&c) {
                    count += 1;
                    chars.next();
                }

                // If we have accumulated text, escape and add it
                if !current_text.is_empty() {
                    result.push_str(&escape_non_formatting_chars(&current_text));
                    current_text.clear();
                }

                // Handle formatting markers
                match (in_format, count) {
                    (None, _) => {
                        // Start formatting
                        in_format = Some(format_type);
                        // Add the formatting characters without escaping
                        for _ in 0..count {
                            result.push(c);
                        }
                    }
                    (Some(current_type), _) if current_type == format_type => {
                        // End formatting
                        in_format = None;
                        // Add the formatting characters without escaping
                        for _ in 0..count {
                            result.push(c);
                        }
                    }
                    _ => {
                        // Mismatched formatting or nested formats - escape the characters
                        for _ in 0..count {
                            result.push('\\');
                            result.push(c);
                        }
                    }
                }
            }
            _ => {
                current_text.push(c);
            }
        }
    }

    // Handle any remaining text
    if !current_text.is_empty() {
        result.push_str(&escape_non_formatting_chars(&current_text));
    }

    result
}

async fn call_xai_api(state: &AppState, question: &str) -> Result<String> {
    let response = state
        .http_client
        .post("https://api.x.ai/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", state.xai_token))
        .json(&json!({
            "messages": [
                {
                    "role": "system",
                    "content": "You are a helpful assistant. When formatting responses:
                    - Use *word* for bold text (surround text with single asterisks)
                    - Start list items with - or *
                    - Keep responses clear and structured
                    - Separate paragraphs with blank lines
                    
                    Example format:
                    Here are the prices:
                    - *Bitcoin (BTC)*: The price is $50,000
                    - *Ethereum (ETH)*: The price is $3,000"
                },
                {
                    "role": "user",
                    "content": question
                }
            ],
            "model": "grok-beta",
            "stream": false,
            "temperature": 0
        }))
        .send()
        .await?
        .json::<Value>()
        .await?;

    Ok(response["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("No response received")
        .to_string())
}

fn format_help_message() -> String {
    format!(
        "*Available Commands:*\n\n\
        📌 */help* \\- Show this help message\n\n\
        📝 */create* \\<name\\> \\<interval\\_minutes\\> \\<question\\>\n\
        Creates a recurring X\\.AI query task\n\
        Example: `/create weather 60 What's the weather in New York?`\n\n\
        📋 */list* \\- Show all active tasks\n\n\
        🗑 */delete* \\<name\\> \\- Remove a task\n\n\
        ❓ */ask* \\<question\\> \\- Ask X\\.AI a one\\-time question"
    )
}

fn format_task_list(tasks: &[sqlx::sqlite::SqliteRow]) -> String {
    if tasks.is_empty() {
        return String::from("📭 *No tasks found*");
    }

    let mut formatted = String::from("*📋 Active Tasks:*\n\n");

    for task in tasks {
        formatted.push_str(&format!(
            "🔷 *Task:* {}\n\
            📝 *Question:* `{}`\n\
            ⏱ *Interval:* {} minutes\n\
            🕒 *Last run:* _{}_\n\n",
            escape_markdown_v2(&task.get::<String, _>("name")),
            escape_markdown_v2(&task.get::<String, _>("question")),
            task.get::<i64, _>("interval"),
            escape_markdown_v2(&task.get::<String, _>("last_run"))
        ));
    }

    formatted
}

async fn create_task(
    pool: &SqlitePool,
    name: &str,
    question: &str,
    interval: i64,
    chat_id: i64,
) -> Result<(), BotError> {
    sqlx::query(
        "INSERT INTO tasks (name, description, interval, last_run, chat_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(name)
    .bind(question)
    .bind(interval)
    .bind(Utc::now().to_rfc3339())
    .bind(chat_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn delete_task(pool: &SqlitePool, name: &str, chat_id: i64) -> Result<bool, BotError> {
    let result = sqlx::query("DELETE FROM tasks WHERE name = ? AND chat_id = ?")
        .bind(name)
        .bind(chat_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

async fn try_send_message(bot: &Bot, chat_id: ChatId, message: String) -> Result<(), BotError> {
    bot.send_message(chat_id, message)
        .parse_mode(ParseMode::MarkdownV2)
        .await
        .map_err(BotError::TelegramError)?;
    Ok(())
}

async fn handle_command(bot: Bot, msg: Message, cmd: Command, state: State) -> ResponseResult<()> {
    let start_time = std::time::Instant::now();
    let cmd_str = format!("{:?}", cmd);
    
    let user_id = msg.from.as_ref().map(|user| user.id.0.try_into().unwrap());
    let username = msg.from.as_ref().and_then(|user| user.username.clone());

    let result = async {
        match cmd {
            Command::Create(args) => {
                match parse_create_command(args).await {
                    Some((name, interval, question)) => {
                        call_xai_api(&state, &question).await?;
                        
                        create_task(&state.pool, &name, &question, interval as i64, msg.chat.id.0).await?;
                        
                        let create_message = format!(
                            "✅ *Task Created Successfully*\n\n\
                            📌 *Name:* {}\n\
                            ❓ *Question:* `{}`\n\
                            ⏱ *Interval:* {} minutes\n\n\
                            🔄 First response coming shortly\\.\\.\\.",
                            escape_markdown_v2(&name), 
                            escape_markdown_v2(&question), 
                            interval
                        );
                        
                        try_send_message(&bot, msg.chat.id, create_message).await?;

                        if let Ok(initial_response) = call_xai_api(&state, &question).await {
                            let formatted_response = format_xai_response(Some(&name), &question, &initial_response);
                            try_send_message(&bot, msg.chat.id, formatted_response).await?;
                        }
                    }
                    None => return Err(BotError::InvalidParameters),
                }
            },
            Command::List => {
                let tasks = sqlx::query(
                    "SELECT name, description as question, interval, last_run FROM tasks WHERE chat_id = ?"
                )
                .bind(msg.chat.id.0)
                .fetch_all(&state.pool)
                .await?;

                let message = format_task_list(&tasks);
                try_send_message(&bot, msg.chat.id, message).await?;
            },
            Command::Delete(name) => {
                if delete_task(&state.pool, &name, msg.chat.id.0).await? {
                    try_send_message(
                        &bot, 
                        msg.chat.id, 
                        format!("✅ Task *{}* deleted successfully", escape_markdown_v2(&name))
                    ).await?;
                } else {
                    return Err(BotError::TaskNotFound);
                }
            },
            Command::Ask(question) => {
                let response = call_xai_api(&state, &question).await?;
                let formatted = format_xai_response(None, &question, &response);
                try_send_message(&bot, msg.chat.id, formatted).await?;
            },
            Command::Help => {
                try_send_message(&bot, msg.chat.id, format_help_message()).await?;
            },
            Command::MyId => {
                if let Some(user) = &msg.from {
                    let is_creator = user.id.0 as i64 == state.owner_id;  // Simplified check
                    let user_info = format!(
                        "👤 *Your Telegram Info:*\n\n\
                        🆔 *User ID:* `{}`\n\
                        📝 *Username:* @{}\n\
                        👑 *Bot Owner:* {}\n",
                        user.id,
                        user.username.as_deref().unwrap_or("none"),
                        if is_creator { "Yes ✅" } else { "No ❌" }
                    );
                    try_send_message(&bot, msg.chat.id, user_info).await?;
                }
            },
            Command::BotStats => {
                if let Some(user_id) = user_id {
                    if user_id == state.owner_id {  // Direct comparison
                        match get_command_stats(&state.pool).await {
                            Ok(stats) => {
                                let formatted_stats = format_bot_stats(&stats);
                                try_send_message(&bot, msg.chat.id, formatted_stats).await?;
                            }
                            Err(e) => {
                                log::error!("Failed to get bot stats: {}", e);
                                return Err(BotError::DatabaseError(e));
                            }
                        }
                    } else {
                        return Err(BotError::PermissionDenied);
                    }
                }
            },
            Command::Stats => {
                if let Some(user_id) = user_id {
                    match get_user_stats(&state.pool, user_id).await {
                        Ok(stats) => {
                            let formatted_stats = format_user_stats(&stats);
                            try_send_message(&bot, msg.chat.id, formatted_stats).await?;
                        }
                        Err(e) => {
                            log::error!("Failed to get user stats: {}", e);
                            return Err(BotError::DatabaseError(e));
                        }
                    }
                }
            },
        }
        Ok(())
    }.await;

    // Log the interaction after command execution
    if let Some(uid) = user_id {
        let _ = log_interaction(
            &state.pool,
            msg.chat.id.0,
            Some(uid),
            username,
            &cmd_str,
            None,
            None,
            result.as_ref().err().map(|e| e.to_string()).as_deref(),
            start_time.elapsed(),
        )
        .await
        .map_err(|e| log::error!("Failed to log interaction: {}", e));
    }

    match result {
        Ok(_) => Ok(()),
        Err(err) => {
            let _ = try_send_message(&bot, msg.chat.id, err.user_message()).await;
            log::error!("Command error: {:?}", err);
            Ok(())
        }
    }
}


async fn check_and_run_tasks(state: State) -> Result<(), BotError> {
    let now = Utc::now();
    let tasks =
        sqlx::query("SELECT name, description as question, interval, last_run, chat_id FROM tasks")
            .fetch_all(&state.pool)
            .await?;

    for task in tasks {
        let last_run: DateTime<Utc> = task.get::<String, _>("last_run").parse()?;
        let interval: i64 = task.get("interval");
        let duration_since_last = now.signed_duration_since(last_run);

        if duration_since_last.num_minutes() >= interval {
            let name: String = task.get("name");
            let question: String = task.get("question");
            let chat_id: i64 = task.get("chat_id");

            log::info!("Running task '{}' with question: {}", name, question);

            match call_xai_api(&state, &question).await {
                Ok(response) => {
                    let formatted_response = format_xai_response(Some(&name), &question, &response);
                    let bot = Bot::new(&env::var("TELEGRAM_BOT_TOKEN").unwrap());
                    if let Err(e) =
                        try_send_message(&bot, ChatId(chat_id), formatted_response).await
                    {
                        log::error!("Failed to send task response: {:?}", e);
                        continue;
                    }
                }
                Err(e) => {
                    log::error!("Failed to get X.AI response for task {}: {:?}", name, e);
                    continue;
                }
            }

            sqlx::query("UPDATE tasks SET last_run = ? WHERE name = ?")
                .bind(now.to_rfc3339())
                .bind(&name)
                .execute(&state.pool)
                .await?;
        }
    }
    Ok(())
}

async fn try_connect_bot(token: &str, retries: u32, delay: Duration) -> Result<Bot, BotError> {
    let mut attempt = 0;
    loop {
        match Bot::new(token).get_me().await {
            Ok(_) => {
                log::info!("Successfully connected to Telegram API");
                return Ok(Bot::new(token));
            }
            Err(e) => {
                attempt += 1;
                if attempt >= retries {
                    return Err(BotError::TelegramError(e));
                }
                log::warn!(
                    "Failed to connect to Telegram API (attempt {}/{}): {:?}",
                    attempt,
                    retries,
                    e
                );
                sleep(delay).await;
            }
        }
    }
}

async fn run_bot(bot: Bot, state: State) -> Result<(), BotError> {
    let handler = move |bot: Bot, msg: Message, cmd: Command| {
        handle_command(bot, msg, cmd, Arc::clone(&state))
    };

    // Remove the ? operator since Command::repl returns ()
    Command::repl(bot, handler).await;
    Ok(())
}

async fn run_with_retry(state: State, telegram_token: String) {
    let retry_delay = Duration::from_secs(5);
    let max_retries = 5;

    loop {
        log::info!("Attempting to start bot...");

        match try_connect_bot(&telegram_token, max_retries, retry_delay).await {
            Ok(bot) => match run_bot(bot, Arc::clone(&state)).await {
                Ok(_) => {
                    log::info!("Bot stopped gracefully");
                    break;
                }
                Err(e) => {
                    log::error!(
                        "Bot crashed: {:?}. Restarting in {} seconds...",
                        e,
                        retry_delay.as_secs()
                    );
                    sleep(retry_delay).await;
                }
            },
            Err(e) => {
                log::error!(
                    "Failed to connect to Telegram API after {} attempts: {:?}",
                    max_retries,
                    e
                );
                log::info!("Retrying in {} seconds...", retry_delay.as_secs());
                sleep(retry_delay).await;
            }
        }
    }
}

fn format_bot_stats(stats: &Value) -> String {
    let mut formatted = String::from("*📊 Bot Usage Statistics*\n\n");
    
    if let Some(commands) = stats["commands"].as_array() {
        for cmd in commands {
            formatted.push_str(&format!(
                "🔷 *{}*\n\
                  ├ Usage Count: {}\n\
                  ├ Avg Response: {:.2}ms\n\
                  └ Error Rate: {:.2}%\n\n",
                escape_markdown_v2(cmd["command"].as_str().unwrap_or("unknown")),
                cmd["usage_count"].as_i64().unwrap_or(0),
                escape_markdown_v2(&format!("{:.2}", cmd["avg_execution_time_ms"].as_f64().unwrap_or(0.0))),
                escape_markdown_v2(&format!("{:.2}", cmd["error_rate"].as_f64().unwrap_or(0.0)))
            ));
        }
    }

    formatted
}

fn format_user_stats(stats: &Value) -> String {
    format!(
        "*📊 Your Usage Statistics*\n\n\
        📈 *Total Commands:* {}\n\
        📅 *Active Days:* {}\n\
        ⚡ *Average Response Time:* {}\n\
        ❌ *Error Rate:* {}",
        stats["total_commands"].as_i64().unwrap_or(0),
        stats["active_days"].as_i64().unwrap_or(0),
        escape_markdown_v2(&format!("{:.2}ms", stats["avg_execution_time_ms"].as_f64().unwrap_or(0.0))),
        escape_markdown_v2(&format!("{:.2}%", stats["error_rate"].as_f64().unwrap_or(0.0)))
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();

    log::info!("Starting task bot...");

    let telegram_token = env::var("TELEGRAM_BOT_TOKEN")
        .context("TELEGRAM_BOT_TOKEN not found in environment variables or .env file")?;
    let xai_token = env::var("XAI_API_TOKEN")
        .context("XAI_API_TOKEN not found in environment variables or .env file")?;
    
    // Add owner ID initialization
    let owner_id = env::var("BOT_OWNER_ID")
        .context("BOT_OWNER_ID not found in environment variables or .env file")?
        .parse::<i64>()
        .context("BOT_OWNER_ID must be a valid integer")?;

    initialize_database().await?;

    let db_path = Path::new("data").join("tasks.db");
    let database_url = format!("sqlite:{}", db_path.to_string_lossy());

    let pool = SqlitePool::connect(&database_url)
        .await
        .context("Failed to connect to SQLite database")?;

    let state = Arc::new(AppState {
        pool,
        http_client: Client::new(),
        xai_token,
        owner_id,
    });

    let state_clone = Arc::clone(&state);

    tokio::spawn(async move {
        loop {
            if let Err(e) = check_and_run_tasks(Arc::clone(&state_clone)).await {
                log::error!("Error checking tasks: {}", e);
            }
            sleep(Duration::from_secs(60)).await;
        }
    });

    log::info!("Bot started successfully!");

    run_with_retry(state, telegram_token).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[test]
    fn test_escape_markdown_v2() {
        let input = "Hello *world* with [link] and (parens)";
        let escaped = escape_markdown_v2(input);
        assert_eq!(escaped, r"Hello \*world\* with \[link\] and \(parens\)");
    }

    #[test]
    fn test_format_response_content() {
        // Test list formatting with debug output
        let list_input = "Items:\n- First item\n- *Second* item";
        let formatted = format_response_content(list_input);
        println!("Formatted output: {}", formatted);
        
        // Test list items - asterisks are preserved for formatting
        assert!(formatted.contains("• First item")); 
        assert!(formatted.contains("• *Second* item")); // Markdown formatting is preserved
    
        // Test paragraph formatting
        let text_with_formatting = "Here is *bold* and `code` text";
        let formatted_text = format_response_content(text_with_formatting);
        assert!(formatted_text.contains("Here is *bold* and `code` text")); // Markdown formatting is preserved
    
        // Test multiple paragraphs with lists
        let multi_paragraph = "First paragraph\n\nList:\n- Item 1\n- *Item* 2\n\nLast paragraph";
        let formatted_multi = format_response_content(multi_paragraph);
        assert!(formatted_multi.contains("First paragraph"));
        assert!(formatted_multi.contains("• Item 1"));
        assert!(formatted_multi.contains("• *Item* 2")); // Markdown formatting is preserved
        assert!(formatted_multi.contains("Last paragraph"));
    
        // Test special characters are escaped but formatting is preserved
        let mixed_content = "Here's a *bold* statement with some (parentheses)";
        let formatted_mixed = format_response_content(mixed_content);
        assert!(formatted_mixed.contains("Here\\'s a *bold* statement with some \\(parentheses\\)")); // Special chars escaped, formatting preserved
    }

    #[test]
    fn test_format_xai_response() {
        let question = "What's the price?";
        let response = "Bitcoin is at $50,000";

        // Test with task name
        let with_task = format_xai_response(Some("price_check"), question, response);
        assert!(with_task.contains("price\\_check"));
        assert!(with_task.contains("What\\'s the price\\?"));
        assert!(with_task.contains("Bitcoin is at \\$50\\,000"));

        // Test without task name
        let without_task = format_xai_response(None, question, response);
        assert!(!without_task.contains("Task:"));
        assert!(without_task.contains("Question:"));
        assert!(without_task.contains("Answer:"));
    }

    #[test]
    fn test_help_message() {
        let help = format_help_message();
        assert!(help.contains("/help"));
        assert!(help.contains("/create"));
        assert!(help.contains("/list"));
        assert!(help.contains("/delete"));
        assert!(help.contains("/ask"));
    }

    #[tokio::test]
    async fn test_database_operations() -> Result<()> {
        // Setup in-memory database for testing
        let pool = SqlitePool::connect("sqlite::memory:").await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                interval INTEGER NOT NULL,
                last_run TEXT NOT NULL,
                chat_id INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Test task creation
        let result = sqlx::query(
            "INSERT INTO tasks (name, description, interval, last_run, chat_id) VALUES (?, ?, ?, ?, ?)"
        )
        .bind("test_task")
        .bind("test description")
        .bind(60)
        .bind(Utc::now().to_rfc3339())
        .bind(123456789)
        .execute(&pool)
        .await;

        assert!(result.is_ok());

        // Test task retrieval
        let task = sqlx::query("SELECT * FROM tasks WHERE name = ?")
            .bind("test_task")
            .fetch_one(&pool)
            .await?;

        assert_eq!(task.get::<String, _>("name"), "test_task");
        assert_eq!(task.get::<i64, _>("interval"), 60);

        // Test task deletion
        let delete_result = sqlx::query("DELETE FROM tasks WHERE name = ?")
            .bind("test_task")
            .execute(&pool)
            .await?;

        assert_eq!(delete_result.rows_affected(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_task_scheduling() -> Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                interval INTEGER NOT NULL,
                last_run TEXT NOT NULL,
                chat_id INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        let now = Utc::now();

        // Create a task that should run
        sqlx::query(
            "INSERT INTO tasks (name, description, interval, last_run, chat_id) VALUES (?, ?, ?, ?, ?)"
        )
        .bind("schedule_test")
        .bind("test description")
        .bind(1) // 1 minute interval
        .bind(now.checked_sub_signed(chrono::Duration::minutes(2)).unwrap().to_rfc3339())
        .bind(123456789)
        .execute(&pool)
        .await?;

        // Check if task should run
        let task = sqlx::query("SELECT * FROM tasks WHERE name = ?")
            .bind("schedule_test")
            .fetch_one(&pool)
            .await?;

        let last_run: DateTime<Utc> = task.get::<String, _>("last_run").parse()?;
        let interval: i64 = task.get("interval");
        let duration_since_last = now.signed_duration_since(last_run);

        assert!(duration_since_last.num_minutes() >= interval);

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_create_command() {
        let valid_input = "test_task 30 What is the weather?".to_string();
        let result = parse_create_command(valid_input).await;
        assert!(result.is_some());

        if let Some((name, interval, question)) = result {
            assert_eq!(name, "test_task");
            assert_eq!(interval, 30);
            assert_eq!(question, "What is the weather?");
        }

        let invalid_input = "invalid command".to_string();
        let result = parse_create_command(invalid_input).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_format_task_list() -> Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                interval INTEGER NOT NULL,
                last_run TEXT NOT NULL,
                chat_id INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        let timestamp = "2024-02-20T12:00:00Z";
        
        sqlx::query(
            "INSERT INTO tasks (name, description, interval, last_run, chat_id) VALUES (?, ?, ?, ?, ?)"
        )
        .bind("test_task")
        .bind("What is the weather?")
        .bind(30)
        .bind(timestamp)
        .bind(123456789)
        .execute(&pool)
        .await?;

        let tasks = sqlx::query("SELECT name, description as question, interval, last_run FROM tasks")
            .fetch_all(&pool)
            .await?;

        let formatted = format_task_list(&tasks);

        assert!(formatted.contains("test\\_task"));
        assert!(formatted.contains("30 minutes"));
        assert!(formatted.contains("What is the weather\\?"));
        assert!(formatted.contains(&escape_markdown_v2(timestamp)));

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_task_list() -> Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                interval INTEGER NOT NULL,
                last_run TEXT NOT NULL,
                chat_id INTEGER NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        let tasks =
            sqlx::query("SELECT name, description as question, interval, last_run FROM tasks")
                .fetch_all(&pool)
                .await?;

        let formatted = format_task_list(&tasks);
        assert!(formatted.contains("No tasks found"));

        Ok(())
    }

    #[test]
    fn test_markdown_escaping() {
        let special_chars = "._*[]()~`>#+-=|{}.!";
        let escaped = escape_markdown_v2(special_chars);
        assert_eq!(escaped, r"\.\_\*\[\]\(\)\~\`\>\#\+\-\=\|\{\}\.\!");

        // Test individual characters
        assert_eq!(escape_markdown_v2("."), r"\.");
        assert_eq!(escape_markdown_v2("*"), r"\*");
        assert_eq!(escape_markdown_v2("_"), r"\_");
        assert_eq!(escape_markdown_v2("["), r"\[");
        assert_eq!(escape_markdown_v2("]"), r"\]");
        assert_eq!(escape_markdown_v2("("), r"\(");
        assert_eq!(escape_markdown_v2(")"), r"\)");
        assert_eq!(escape_markdown_v2("~"), r"\~");
        assert_eq!(escape_markdown_v2("`"), r"\`");
        assert_eq!(escape_markdown_v2(">"), r"\>");
        assert_eq!(escape_markdown_v2("#"), r"\#");
        assert_eq!(escape_markdown_v2("+"), r"\+");
        assert_eq!(escape_markdown_v2("-"), r"\-");
        assert_eq!(escape_markdown_v2("="), r"\=");
        assert_eq!(escape_markdown_v2("|"), r"\|");
        assert_eq!(escape_markdown_v2("{"), r"\{");
        assert_eq!(escape_markdown_v2("}"), r"\}");
        assert_eq!(escape_markdown_v2("!"), r"\!");
    }

    #[tokio::test]
    async fn test_create_command_validation() {
        // Valid command
        let valid = parse_create_command("weather 60 What's the weather like?".to_string()).await;
        assert!(valid.is_some());
        if let Some((name, interval, question)) = valid {
            assert_eq!(name, "weather");
            assert_eq!(interval, 60);
            assert_eq!(question, "What's the weather like?");
        }

        // Invalid commands
        let invalid_cases = vec![
            "weather".to_string(),
            "weather 60".to_string(),
            "weather invalid 60".to_string(),
            "".to_string(),
        ];

        for case in invalid_cases {
            assert!(parse_create_command(case).await.is_none());
        }
    }

    #[test]
    fn test_xai_response_formatting() {
        let response = format_xai_response(
            Some("crypto_check"),
            "What's the BTC price?",
            "Bitcoin is at $50,000"
        );

        assert!(response.contains("crypto\\_check"));
        assert!(response.contains("What\\'s the BTC price\\?"));
        assert!(response.contains("Bitcoin is at \\$50\\,000"));

        let without_task = format_xai_response(
            None, 
            "What's the BTC price?",
            "Bitcoin is at $50,000"
        );

        assert!(!without_task.contains("Task:"));
        assert!(without_task.contains("Question:"));
        assert!(without_task.contains("Answer:"));
    }

    #[test]
    fn test_special_character_escaping() {
        let text = "What's this? It's a test!";
        let escaped = escape_markdown_v2(text);
        assert_eq!(escaped, r"What\'s this\? It\'s a test\!");
    }
}
