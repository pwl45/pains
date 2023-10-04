# spur
stock price utility in rust (spur)

## Installation
```
git clone https://github.com/pwl45/spur
cd spur
cargo install --path .
```

## Usage

You can get the price of a stock with `spur <ticker>`, for example

`spur GOOG`

to get the price of Google.

Stock tickers are case-insensitive. If you want to specify an exchange, you can do so with 

`spur <ticker>:<exchange>`

e.g. 

`spur GOOG:NASDAQ`
