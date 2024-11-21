# Wibot - X.AI Integration Telegram Bot
A Telegram bot that integrates with X.AI API to schedule and automate AI-powered queries.
[![License: MIT/Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

## Features
- Schedule recurring X.AI queries
- Real-time responses from X.AI API
- Automatic task execution at specified intervals
- SQLite database for persistent task storage
- Docker support for easy deployment
- Proper error handling and retry mechanisms
- Markdown formatting support for responses
- Comprehensive usage statistics tracking

## Try it Live! 🤖
You can test the bot right now on Telegram:
[@GrokWiBot](https://t.me/GrokWiBot)

Experience all features firsthand and see how it can help streamline your AI interactions!

> **📊 Privacy Notice**: @GrokWiBot collects and stores usage statistics including command execution times, success rates, and user IDs to enable the `/stats` feature. If you prefer not to have your usage statistics saved, please do not use the bot. All stored data is used solely for providing usage insights through the `/stats` command.

## Prerequisites
- Docker and Docker Compose
- Telegram Bot Token (from [@BotFather](https://t.me/botfather))
- X.AI API Token

## Quick Start
1. Clone the repository:
```bash
git clone https://github.com/sundayglee/wibot.git
cd wibot
```

2. Create a `.env` file with your tokens:
```bash
TELEGRAM_BOT_TOKEN=your_telegram_bot_token
XAI_API_TOKEN=your_xai_api_token
RUST_LOG=info
```

3. Build and run using Docker:
```bash
docker-compose up -d --build
```

## Usage
The bot supports the following commands:
- `/help` - Show available commands
- `/create <name> <interval_minutes> <question>` - Create a recurring X.AI query task
- `/list` - Show all active tasks
- `/delete <name>` - Delete a task
- `/ask <question>` - Ask X.AI a one-time question
- `/stats` - View your personal usage statistics
- `/myid` - Show your Telegram ID and bot owner status
- `/botstats` - View overall bot usage statistics (bot owner only)


Example:
```
/create crypto 30 "What's the current price of Bitcoin and Ethereum?"
```

### Statistics Commands 📊

**Personal Statistics**:
```
# View your command usage stats
/stats
Response:
📊 Your Usage Statistics
📈 Total Commands: 42
📅 Active Days: 7
⚡ Average Response Time: 245.32ms
❌ Error Rate: 0.15%

# Check your Telegram ID and owner status
/myid
Response:
👤 Your Telegram Info:
🆔 User ID: 123456789
📝 Username: @username
👑 Bot Owner: No ❌
```

**Bot Owner Commands**:
```
# View overall bot performance metrics
/botstats
Response:
📊 Bot Usage Statistics

🔷 /ask
├ Usage Count: 156
├ Avg Response: 234.56ms
└ Error Rate: 0.12%

🔷 /create
├ Usage Count: 45
├ Avg Response: 189.23ms
└ Error Rate: 0.05%

[... other commands ...]
```

## Examples and Use Cases 🎯

Here are some practical examples of how to use the bot's commands:

### Ask One-time Questions 💭
```
# Get current news
/ask What are the latest major news headlines?

# Get weather updates
/ask What's the weather like in New York today?

# Technical analysis
/ask Analyze the current Bitcoin price trend and provide technical analysis

# Code help
/ask How do I implement a binary search tree in Python?

# Language help
/ask Translate "Hello, how are you?" to Spanish, French, and German
```

### Create Recurring Tasks ⏰
```
# Monitor Elon Musk's posts
/create elon_updates 30 "What are the latest posts and announcements from @elonmusk?"

# Crypto price tracking
/create crypto_watch 15 "What's the current price of Bitcoin, Ethereum, and Solana? Give me a brief market analysis"

# Weather updates
/create weather_nyc 60 "What's the weather forecast for New York City today? Include temperature and precipitation chances"

# News digest
/create news_digest 180 "Summarize the most important world news from the last 3 hours"

# Stock market updates
/create stock_check 30 "What are the current prices of AAPL, GOOGL, and TSLA? Any significant movements?"

# AI industry news
/create ai_news 360 "What are the latest significant developments in AI and machine learning?"

# Coding tips
/create coding_tip 720 "Share a Rust programming tip or best practice"
```

### List Management 📋
```
# Show all tasks
/list

# Remove specific tasks
/delete elon_updates
/delete crypto_watch
/delete weather_nyc
```

### Statistics Features 📉

**Personal Statistics** (`/stats`):
- Total commands executed
- Number of active usage days
- Average command response time
- Personal error rate

**Bot Statistics** (`/botstats`, owner only):
- Per-command usage counts
- Command-specific response times
- Error rates by command type
- Overall bot performance metrics

### Pro Tips 💡
1. **Performance Monitoring**:
   - Check `/stats` regularly to monitor your usage patterns
   - Use response times to schedule intensive tasks during off-peak hours

2. **Error Tracking**:
   - Monitor error rates to identify problematic commands
   - If errors increase, try adjusting query complexity or timing

3. **Usage Optimization**:
   - Review command statistics to optimize task intervals
   - Identify and remove unused recurring tasks
   
4. **Interval Selection**:
   - 15-30 minutes: For time-sensitive data (crypto prices, breaking news)
   - 60-180 minutes: For regular updates (weather, general news)
   - 360-720 minutes: For daily digests and less time-sensitive information

5. **Task Naming**:
   - Use descriptive names: `crypto_btc`, `news_tech`, `weather_london`
   - Avoid spaces in task names
   - Use underscores for multi-word tasks

6. **Query Optimization**:
   - Be specific in your questions
   - Specify the format you want (e.g., "provide the answer in bullet points")
   - Include time frame if relevant (e.g., "in the last 2 hours")

### Real-world Scenarios 🌍

**Crypto Trader Setup**:
```
/create btc_alert 15 "Has Bitcoin's price changed more than 5% in the last hour? If yes, provide technical analysis"
/create eth_check 30 "What's Ethereum's current price and trading volume? Compare with 24h average"
/create market_mood 60 "Analyze the overall crypto market sentiment. Include major news affecting prices"
```

**News Monitoring**:
```
/create tech_news 180 "What are the latest significant announcements in the tech industry? Focus on AI, crypto, and startups"
/create science_update 360 "What are the latest breakthrough scientific discoveries or research papers?"
```

**Weather Watcher**:
```
/create rain_alert 30 "Is it going to rain in London in the next 2 hours? Give percentage chance"
/create weather_weekly 720 "What's the weather forecast for New York for the next 7 days? Include temperature ranges"
```

**Social Media Tracker**:
```
/create elon_tracker 30 "What are @elonmusk's latest tweets about Tesla, SpaceX, or X?"
/create crypto_social 60 "What are the trending topics in crypto Twitter? Include major influencer discussions"
```

**Market Analysis**:
```
/create market_open 60 "What are the major pre-market movers today? Focus on tech stocks"
/create forex_update 30 "What's the current EUR/USD and GBP/USD rate? Include brief technical analysis"
```

## Development
### Requirements
- Rust 1.75 or later
- SQLite 3
- OpenSSL 3.0

### Building from Source
1. Install dependencies:
```bash
# Debian/Ubuntu
sudo apt-get update
sudo apt-get install -y build-essential sqlite3 libsqlite3-dev pkg-config libssl-dev
```

2. Build the project:
```bash
cargo build --release
```

3. Run:
```bash
RUST_LOG=info ./target/release/wibot
```

## Configuration
The bot is configured via environment variables:
- `TELEGRAM_BOT_TOKEN`: Your Telegram bot token
- `XAI_API_TOKEN`: Your X.AI API token
- `RUST_LOG`: Logging level (info, debug, error)

## Project Structure
```
wibot/
├── src/
│   └── main.rs
├── data/
│   └── tasks.db
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── README.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE-MIT
└── LICENSE-APACHE
```

## Support the Development ☕

If you find this bot useful and would like to support its continued development and API costs, consider buying me a virtual coffee! Your support helps keep the servers running and enables new features.

**₿ Bitcoin Donation Address:**
```
19VzmbqAr6bKUBhCADLxP1NLMDd6dxqVKz
```

<p align="center">
  <img src="https://img.icons8.com/color/48/000000/bitcoin--v1.png" alt="Bitcoin"/>
  <br/>
  Every satoshi helps power AI interactions!
</p>

Your contributions help with:
- 🔋 Keeping the bot running 24/7
- 💡 Developing new features
- 🚀 Maintaining API access
- 🛠 Infrastructure costs
- 📈 Scaling the service

Thank you for your support! Together, we can make AI interactions more accessible to everyone! 🙏

## Contributing
We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Testing
### Running All Tests
```bash
cargo test
```

### Test Coverage
To generate test coverage report:
```bash
cargo install cargo-tarpaulin
cargo tarpaulin
```

## License
This project is dual-licensed under either:
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
at your option.

## Changelog
See [CHANGELOG.md](CHANGELOG.md) for version history.

## Security
This application is not tested or passed any security audit. Use it at your own risks.