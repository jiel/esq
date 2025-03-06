🏎️ esq – the minimalist CLI to query Elasticsearch.

esq is an opinionated, minimalist and blazing-fast command-line tool to query and manipulate logs in Elasticsearch. It's designed to be a small, composable tool that works well with others.

🚀 Speed First · 🛠 Use the Right Tools · 🔥 Minimalism Over Complexity

## 🎯 Philosophy

🐧 Linux-style simplicity – esq does one thing well: fetching logs fast.  
🔗 Composability – Use it with your favorite command line tools
♻️ No reinventing the wheel – esq relies on [dateparser](https://docs.rs/dateparser/latest/dateparser/) for date parsing.  
💡 You won't find fancy built-in JSON parsing or log transformations here. Use the json swiss army knife aka JQ to process output.  

## 🚀 Installation

### From Source
```bash
cargo install --git https://github.com/jiel/esq
```

### Binary Releases
Download the latest release for your platform from the [releases page](https://github.com/jiel/esq/releases).

## 📝 Usage

```bash
# Login to your instance
esq login

# list available indexes
esq ls

# Basic usage - fetch recent logs from an index
esq cat my-logs-index

# Fetch logs around a specific time
esq cat my-logs-index --around "2:00pm"

# Fetch logs from a specific time range
esq cat my-logs-index --from "10:00:00" --to "10:00:30"

# Follow logs in real-time (like tail -f)
esq cat my-logs-index --follow

# Select specific fields only
esq cat my-logs-index --select "timestamp,message,level"

# Get more logs
esq cat my-logs-index -n 10000
```

## 🔄 Composability Examples

### Filter logs with jq
```bash
esq cat my-logs-index | jq 'select(.level == "ERROR")'
```

### Count logs by level
```bash
esq cat my-logs-index | jq -r '.level' | sort | uniq -c
```

### Extract structured data and convert to CSV
```bash
esq cat my-logs-index | jq -r '[.timestamp, .level, .message] | @csv' > logs.csv
```


## 📊 Performance Tips

- Use the `--select` option to fetch only the fields you need
- Process logs in batches (reasonable `-n` values) for better performance
- For time-based queries, use narrower time ranges when possible

## 👨‍💻 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🙏 Acknowledgements

- [clap](https://github.com/clap-rs/clap) for powerful command-line argument parsing
- [dateparser](https://docs.rs/dateparser/latest/dateparser/) for flexible date parsing
- The Rust ecosystem for making blazing-fast tools possible
- Elasticsearch for their powerful search capabilities

---

Built with ❤️ by JLT
