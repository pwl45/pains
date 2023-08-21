use thiserror::Error;
use reqwest::header;
extern crate rand;
use rand::seq::SliceRandom;

struct Stock {
    ticker: String,
    exchange: String,
}

impl Stock {
    fn new(ticker: &str, exchange: &str) -> Stock {
        Stock {
            ticker: ticker.to_string(),
            exchange: exchange.to_string(),
        }
    }
}


#[derive(Error, Debug)]
pub enum StockScrapeError {
    #[error("Failed to fetch the URL: {0}")]
    FetchError(reqwest::Error),

    #[error("Failed to read the response: {0}")]
    ReadError(reqwest::Error),

    #[error("Failed to parse the selector")]
    SelectorParseError,

    #[error("No match found")]
    NoMatchError,
}

// Just try one selector
fn get_price_simple_selector<F>(
    // ticker: &str,
    full_url: &str,
    selector: &str,
    mapper: F
) -> Result<String, StockScrapeError>
where
    F: FnOnce(&str) -> String,
{
    get_price_multiple_selector(full_url,&[selector],mapper)
}

// Try multiple selectors and return the first match
fn get_price_multiple_selector<F>(
    // ticker: &str,
    full_url: &str,
    selectors: &[&str], 
    mapper: F
) -> Result<String, StockScrapeError>
where
    F: FnOnce(&str) -> String,
{
    let client = reqwest::blocking::Client::new();
    println!("{}",full_url);
    let response=client.get(full_url).header(header::USER_AGENT,"Mozilla/5.0 (X11; Linux x86_64; rv:60.0) Gecko/20100101 Firefox/81.0").send()
        .map_err(StockScrapeError::FetchError)?
        .text()
        .map_err(StockScrapeError::ReadError)?;

    let document = scraper::Html::parse_document(&response);
    // println!("{}",response);

    for selector in selectors {
        let title_selector = scraper::Selector::parse(selector)
            .map_err(|_| StockScrapeError::SelectorParseError)?;

        if let Some(element) = document.select(&title_selector).next() {
            let result = element.text().collect::<String>();
            return Ok(mapper(&result));
        }

    }
    Err(StockScrapeError::NoMatchError)
}

fn get_price_yahoo(s: &Stock) -> Result<String, StockScrapeError> {
    let base_url="https://finance.yahoo.com/quote/";
    let full_url = format!("{}{}/", base_url, s.ticker);
    let selector="fin-streamer.Fw\\(b\\).Fz\\(36px\\).Mb\\(-4px\\).D\\(ib\\)";
    
    return get_price_simple_selector(&full_url, selector, |s: &str| s.to_string());
}

fn get_price_salpha(s: &Stock) -> Result<String, StockScrapeError> {
    let base_url="https://seekingalpha.com/symbol/";
    let full_url = format!("{}{}/", base_url, s.ticker);
    let selectors = [
        "span.py-Vf.be-jN.be-j5.be-kb",
        "span.po-UR.aW-jr.aW-jJ.aW-jP",
    ];

    return get_price_multiple_selector(&full_url, &selectors, |s: &str| s.replace("$", ""));
}

fn get_price_googfin(s: &Stock) -> Result<String, StockScrapeError> {
    let base_url="https://www.google.com/finance/quote/";
    let full_url = format!("{}{}:{}", base_url, s.ticker,s.exchange);
    let selector = "div.YMlKec.fxKbKc";

    return get_price_simple_selector(&full_url, selector, |s: &str| s.replace("$", ""));
}

// todo: revisit during business hours
fn get_price_cnbc(s: &Stock) -> Result<String, StockScrapeError> {
    let base_url="https://www.cnbc.com/quotes/";
    let full_url = format!("{}{}", base_url,s.ticker.to_lowercase());
    // let selector = "div.QuoteStrip-dataContainer.QuoteStrip-extendedHours > div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice";
    let selectors = [
        "div.QuoteStrip-dataContainer.QuoteStrip-extendedHours > div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice",
        "div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice",
    ];

    return get_price_multiple_selector(&full_url, &selectors , |s: &str| s.replace("$", ""));
}

// barely works
fn get_price_wsj(s: &Stock) -> Result<String, StockScrapeError> {
    let base_url="https://www.wsj.com/market-data/quotes/";
    let full_url = format!("{}{}", base_url,s.ticker.to_lowercase());
    // let selector = "div.QuoteStrip-dataContainer.QuoteStrip-extendedHours > div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice";
    let selectors = [
        "span#quote_val",
    ];

    return get_price_multiple_selector(&full_url, &selectors , |s: &str| s.replace("$", ""));
}
fn main() {
    let stocks = vec![
        Stock::new("GOOG","NASDAQ"),
        // Stock::new("MSFT","NASDAQ"),
        // Stock::new("SUM","NYSE"),
        Stock::new("SHG","NYSE"),
        // Stock::new("DNPUF","OTCMKTS"),
    ];


    for s in &stocks {
        // println!("{}: {}",s.ticker,get_price_wsj(s).unwrap());
        println!("{}: {}",s.ticker,get_price_yahoo(s).unwrap());
        println!("{}: {}",s.ticker,get_price_salpha(s).unwrap());
        println!("{}: {}",s.ticker,get_price_googfin(s).unwrap());
        println!("{}: {}",s.ticker,get_price_cnbc(s).unwrap());
    }

}

    // let fns = vec![get_price_yahoo,get_price_salpha,get_price_googfin,get_price_cnbc];
    // for s in &stocks {
    //     let stock_fn = fns.choose(&mut rand::thread_rng()).unwrap();
    //     println!("{}: {}",s.ticker,stock_fn(s).unwrap());
    // }
