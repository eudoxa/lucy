# Lucy - Rails log viewer with request_id grouping

Lucy helps you visualize Rails logs by grouping related entries by their request_id.

![Lucy Demo](./docs/lucy-demo.gif)

## Installation

```bash
# Install from local repository
cargo install --path .

# Install from GitHub repository
cargo install --git https://github.com/eudoxa/lucy
```

## Setup
Enable request_id tagging in your Rails application:

```ruby
# config/environments/development.rb (or other environment file)
Rails.application.configure do
  config.log_tags = [:request_id]
end
```

## Usage
Monitor your logs with this command:

```bash
tail -f -n 1000 log/development.log | lucy
```

## Development
To enable debug logs during development, set the `LUCY_DEV` environment variable. This will write debug information to `tracing.log`:

```bash
tail -f -n 1000 your_log_path/development.log | LUCY_DEV=1 cargo run
```
