Log viewer for Rails with request_id

## Install

```
cargo install --path .
```
```
cargo install --git https://github.com/eudoxa/lucy
```

## setup
Add requst_id

```ruby
Rails.application.configure do
  config.log_tags = [:request_id]
end
```

## Run
`tail -f -n 1000 logs/development.log | lucy`
