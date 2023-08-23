extern crate rand;
use thiserror::Error;
use reqwest::header;
use std::collections::HashMap;
use rand::Rng;
use rand::seq::SliceRandom;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use strum_macros::Display;

const STOCK_FNS: [for<'a> fn(&'a Stock) -> Result<String, StockScrapeError>; 5] = [
    get_price_salpha,
    get_price_googfin,
    get_price_yahoo,
    get_price_bloomberg,
    get_price_cnbc,
];

struct StockSource <'a> {
    id: SourceID,
    base_url: String,
    url_transformer: Box<dyn Fn(&str, &Stock) -> String>,
    result_transformer: Box<dyn Fn(&str) -> String>,
    selectors: Vec<&'a str>,
    needs_exchange: bool,
}

#[derive(Debug, EnumIter, Display, Eq, Hash, PartialEq)]
enum SourceID {
    Yahoo,
    SeekingAlpha,
    GoogleFinance,
    WallStreetJournal,
    CNBC,
    Bloomberg,
}

struct Stock {
    ticker: String,
    exchange: Option<String>
}

impl Stock {
    fn new(ticker: &str, exchange: &str) -> Stock {
        Stock {
            ticker: ticker.to_string(),
            exchange: if exchange != "" { Some(exchange.to_string()) } else { None },
        }
    }
    fn new_noex(ticker: &str) -> Stock {
        Stock {
            ticker: ticker.to_string(),
            exchange: None,
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

    #[error("Exchange not provided; data source requires exchange (e.g. NYSE, NASDAQ, etc.)")]
    NoExchangeError,

    #[error("No match found")]
    NoMatchError,
}

#[derive(Debug)]
pub enum AggregatedError {
    AllFailed(Vec<StockScrapeError>),
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
    get_price_multiple_selector(full_url,&vec![selector],mapper)
}

// Try multiple selectors and return the first match
fn get_price_multiple_selector<F>(
    // ticker: &str,
    full_url: &str,
    selectors: &Vec<&str>, 
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
    let selectors = vec![
        r#"[data-test-id="symbol-price"]"#,
    ];

    return get_price_multiple_selector(&full_url, &selectors, |s: &str| s.replace("$", ""));
}

fn get_price_googfin(s: &Stock) -> Result<String, StockScrapeError> {
    let exchange: &String = s.exchange.as_ref().ok_or(StockScrapeError::NoExchangeError)?;
    let base_url="https://www.google.com/finance/quote/";
    let full_url = format!("{}{}:{}", base_url, s.ticker,exchange);
    let selector = "div.YMlKec.fxKbKc";

    return get_price_simple_selector(&full_url, selector, |s: &str| s.replace("$", ""));
}

// todo: revisit during business hours
fn get_price_cnbc(s: &Stock) -> Result<String, StockScrapeError> {
    let base_url="https://www.cnbc.com/quotes/";
    let full_url = format!("{}{}", base_url,s.ticker.to_lowercase());
    // let selector = "div.QuoteStrip-dataContainer.QuoteStrip-extendedHours > div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice";
    let selectors = vec![
        "div.QuoteStrip-dataContainer.QuoteStrip-extendedHours > div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice",
        "div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice",
    ];

    return get_price_multiple_selector(&full_url, &selectors , |s: &str| s.replace("$", ""));
}

fn get_price_wsj(s: &Stock) -> Result<String, StockScrapeError> {
    let base_url="https://www.wsj.com/market-data/quotes/";
    let full_url = format!("{}{}", base_url,s.ticker.to_lowercase());
    let selectors = vec![
        "span#quote_val",
    ];

    return get_price_multiple_selector(&full_url, &selectors , |s: &str| s.replace("$", ""));
}

fn get_price_bloomberg(s: &Stock) -> Result<String, StockScrapeError> {
    let base_url="https://www.bloomberg.com/quote/";
    let full_url = format!("{}{}:US", base_url,s.ticker.to_uppercase());
    // let selector = "div.QuoteStrip-dataContainer.QuoteStrip-extendedHours > div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice";
    let selectors = vec![
        // Match anything that has sized-price as one class and SizedPrice_extraLarge* (class that begins with SizedPrice_extraLarge) as a separate class. 
        r#"div[class~="sized-price"][class*="SizedPrice_extraLarge"]"#
    ];

    return get_price_multiple_selector(&full_url, &selectors , |s: &str| s.replace("$", ""));
}


fn get_price_robust(s: &Stock, source_map: &HashMap<SourceID, fn(&Stock) -> Result<String, StockScrapeError>>) -> Result<String, StockScrapeError> {
    let mut sources: Vec<_> = SourceID::iter().collect();
    // for source in &sources{
    //     println!("{}",source);
    // }
    let mut rng = rand::thread_rng();
    sources.shuffle(&mut rng);
    let mut errs: Vec<StockScrapeError> = vec![];
    // for source in &sources{
    //     println!("{}",source);
    //     println!("{}",source);
    // }
    for source in &sources {
        if let Some(stock_fn)=source_map.get(source){
            let result = stock_fn(s);
            match result {
                Result::Ok(val) => return Ok(val),
                Result::Err(err) => errs.push(err),
            }
        }else{
            //TODO: put something here to handle this case... or refactor so that it cant happen. 
        }
        // println!("{}",s);
    }
    Err(StockScrapeError::NoMatchError)
}

fn get_price_Source(s: &Stock, so: &StockSource) -> Result<String, StockScrapeError> {
    let base_url=&so.base_url;
    let full_url = (so.url_transformer)(&base_url, &s);

    return get_price_multiple_selector(&full_url, &so.selectors , |s: &str| s.replace("$", ""));
}

fn main() {
    let mut source_map: HashMap<SourceID, fn(&Stock) -> Result<String, StockScrapeError>> = HashMap::new();
    let source = StockSource {
            id: SourceID::Yahoo,
            base_url: "https://finance.yahoo.com/quote/".to_string(),
            url_transformer: Box::new(|base_url, stock| format!("{}{}", base_url, stock.ticker)),
            result_transformer: Box::new(|s| s.replace("$", "")),
            selectors: vec!["fin-streamer.Fw\\(b\\).Fz\\(36px\\).Mb\\(-4px\\).D\\(ib\\)"],
            needs_exchange: false,
        };
    source_map.insert(SourceID::GoogleFinance,get_price_googfin);
    source_map.insert(SourceID::SeekingAlpha,get_price_salpha);
    source_map.insert(SourceID::Yahoo,get_price_yahoo);
    source_map.insert(SourceID::Bloomberg,get_price_bloomberg);
    source_map.insert(SourceID::WallStreetJournal,get_price_wsj);
    source_map.insert(SourceID::CNBC,get_price_cnbc);
    let stocks = vec![
        Stock::new("ROIV","NASDAQ"),
        Stock::new("GOOG","NASDAQ"),
        Stock::new("MSFT","NASDAQ"),
        Stock::new("SUM","NYSE"),
        Stock::new("SHG","NYSE"),
        Stock::new("DNPUF","OTCMKTS"),
    ];


    for s in &stocks {
        // println!("{}: {}",s.ticker,get_price_wsj(s).unwrap());
        // println!("{}: {}",s.ticker,get_price_yahoo(s).unwrap());
        // println!("{}: {}",s.ticker,get_price_yahoo(s).unwrap());
        // println!("{}: {}",s.ticker,get_price_bloomberg(s).unwrap());
        // println!("{}: {}",s.ticker,get_price_salpha(s).unwrap());
        // println!("{}: {}",s.ticker,get_price_googfin(s).unwrap());
        println!("{}: {}",s.ticker,get_price_robust(s,&source_map).unwrap());
        println!("{}: {}",s.ticker,get_price_Source(s,&source).unwrap());
    }

    // let fns = vec![get_price_yahoo,get_price_salpha,get_price_googfin,get_price_cnbc];
    // for s in &stocks {
    //     println!("{}: {}",s.ticker,stock_fn(s).unwrap());
    // }
}
