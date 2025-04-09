# Lucy - Rails Log Viewer with Request ID Grouping

Lucy helps you visualize Rails logs by grouping related entries by their request ID.

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

To enable debug logs during development, set the `LUCY_DEV` environment variable:

```bash
LUCY_DEV=1 cargo run
```

Or when using the installed binary:

```bash
tail -f -n 1000 log/development.log | LUCY_DEV=1 lucy
```
