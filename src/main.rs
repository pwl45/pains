extern crate rand;
extern crate regex;
use thiserror::Error;
use reqwest::header;
use rand::seq::SliceRandom;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use strum_macros::Display;

use regex::Regex;

struct StockSource {
    id: SourceID,
    base_url: String,
    url_transformer: fn(&str, &Stock) -> Result<String, StockScrapeError>,
    price_selectors: Vec<String>,
    price_result_transformer: fn(&str) -> String,
    // pct_ch_selectors: Vec<&'a str>,
    pct_ch_selectors: fn(&Stock) -> Vec<String>,
    pct_ch_result_transformer: Box<dyn Fn(&str) -> String>,
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

enum AttrID {
    Price,
    PctCh,
}

struct AttrGettr {
    attr_id: AttrID,
    get_selectors: fn(&Stock) -> Vec<String>,
    result_transformer: fn(&str) -> String,
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
// fn get_price_simple_selector<F>(
//     // ticker: &str,
//     full_url: &str,
//     selector: &str,
//     mapper: F
// ) -> Result<String, StockScrapeError>
// where
//     F: FnOnce(&str) -> String,
// {
//     get_price_multiple_selector(full_url,&vec![selector.to_string()],mapper)
// }

// Try multiple selectors and return the first match
fn get_document(url:&str) -> Result<scraper::Html,StockScrapeError>{

    let client = reqwest::blocking::Client::new();
    println!("{}",url);
    let response=client.get(url).header(header::USER_AGENT,"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/109.0.0.0 Safari/537.36").send()
        .map_err(StockScrapeError::FetchError)?
        .text()
        .map_err(StockScrapeError::ReadError)?;

    return Ok(scraper::Html::parse_document(&response));
}


fn get_price_multiple_selector<F>(
    document: &scraper::Html,
    price_selectors: &Vec<String>, 
    price_selectors_transformer: F
) -> Result<String, StockScrapeError>
where
    F: FnOnce(&str) -> String,
{
    for selector in price_selectors {
        let title_selector = scraper::Selector::parse(selector)
            .map_err(|_| StockScrapeError::SelectorParseError)?;

        if let Some(element) = document.select(&title_selector).next() {
            let result = element.text().collect::<String>();
            return Ok(price_selectors_transformer(&result));
        }

    }
    Err(StockScrapeError::NoMatchError)
}

fn get_price_robust(s: &Stock, sources: &Vec<StockSource>) -> Result<String, StockScrapeError> {
    let mut errs: Vec<StockScrapeError> = vec![];
    for source in sources {
        let result = get_price_source(s,source);
        match result {
            Result::Ok(val) => return Ok(val),
            Result::Err(err) => errs.push(err),
        }
    }
    // for source in sources {
    //     println!("{}",source.id);
    // }
    Err(StockScrapeError::NoMatchError)
}


// fn get_full_price(s: &Stock, so: &StockSource) -> (Result<String, StockScrapeError>, Result<String, StockScrapeError>) {
//     let full_url = (so.url_transformer)(&so.base_url, &s)?;
//     let pct_ch_selectors_applied = (so.pct_ch_selectors)(&s);
//     let document = get_document(&full_url)?;

//     let price_result = get_price_multiple_selector(&document, &so.price_selectors, &so.price_result_transformer);
//     let pct_ch_result = get_price_multiple_selector(&document, &pct_ch_selectors_applied, &so.pct_ch_result_transformer);
//     return (price_result, pct_ch_result);
// }

//TODO: update this function signature to return a pair of Result<String, StockScrapeError>s
fn get_price_source(s: &Stock, so: &StockSource) -> Result<String, StockScrapeError> {
    let full_url = (so.url_transformer)(&so.base_url, &s)?;
    let pct_ch_selectors_applied = (so.pct_ch_selectors)(&s);
    let document = get_document(&full_url)?;

    return get_price_multiple_selector(&document, &so.price_selectors, &so.price_result_transformer);
    //TODO: make this return a pair of Result<String, StockScrapeError> using the commented line below
    // return get_price_multiple_selector(&document, &pct_ch_selectors_applied, &so.pct_ch_result_transformer);
}


fn get_pch_source(s: &Stock, so: &StockSource) -> Result<String, StockScrapeError> {
    let full_url = (so.url_transformer)(&so.base_url, &s)?;
    let pct_ch_selectors_applied = (so.pct_ch_selectors)(&s);
    let document = get_document(&full_url)?;
    return get_price_multiple_selector(&document, &pct_ch_selectors_applied, &so.pct_ch_result_transformer);
}


fn main() {
    let mut sources = vec![
        StockSource {
            id: SourceID::Yahoo,
            base_url: "https://finance.yahoo.com/quote/".to_string(),
            url_transformer: |base_url, stock| Ok(format!("{}{}/", base_url, stock.ticker)),
            price_result_transformer: |s| s.replace("$", ""),
            price_selectors: vec!["fin-streamer.Fw\\(b\\).Fz\\(36px\\).Mb\\(-4px\\).D\\(ib\\)".to_string()],
            pct_ch_selectors: |s: &Stock| {
                let selector = format!(r#"fin-streamer[data-field="regularMarketChangePercent"][data-symbol="{}"]"#,s.ticker);
                vec![selector]
            },
            pct_ch_result_transformer: Box::new(|s| {
                let re = Regex::new(r".*\((.*)\)").unwrap();

                if let Some(captured) = re.captures(s) {
                    if let Some(matched) = captured.get(1) {
                        // println!("{}", matched.as_str());
                        return matched.as_str().to_string();
                    }
                }
                return "".to_string();
            }),
            needs_exchange: false,
        },
        // StockSource {
        //     id: SourceID::SeekingAlpha,
        //     base_url: "https://seekingalpha.com/symbol/".to_string(),
        //     url_transformer: Box::new(|base_url, stock| Ok(format!("{}{}/", base_url, stock.ticker))),
        //     price_result_transformer: Box::new(|s| {s.replace("$", "")}),
        //     price_selectors: vec![r#"[data-test-id="symbol-price"]"#,],
        //     pct_ch_selectors: vec![r#"[data-test-id="symbol-change"]"#,],
        //     needs_exchange: false,
        // },
        //StockSource {
        //    id: SourceID::GoogleFinance,
        //    base_url: "https://www.google.com/finance/quote/".to_string(),
        //    url_transformer: Box::new(|base_url, stock| {
        //        let exchange_str= stock.exchange.as_ref().ok_or(StockScrapeError::NoExchangeError)?;
        //        Ok(format!("{}{}:{}", base_url, stock.ticker,exchange_str))
        //    }),
        //    price_result_transformer: Box::new(|s| s.replace("$", "")),
        //    price_selectors: vec!["div.YMlKec.fxKbKc"],
        //    needs_exchange: true,
        //},
        //StockSource {
        //    id: SourceID::Bloomberg,
        //    base_url: "https://www.bloomberg.com/quote/".to_string(),
        //    url_transformer: Box::new(|base_url, stock| Ok(format!("{}{}:US", base_url,stock.ticker.to_uppercase()))),
        //    price_result_transformer: Box::new(|s| s.replace("$", "")),
        //    // Match anything that has sized-price as one class and SizedPrice_extraLarge* 
        //    //(class that begins with SizedPrice_extraLarge) as a separate class. 
        //    price_selectors: vec![
        //        r#"div[class~="sized-price"][class*="SizedPrice_extraLarge"]"#
        //    ],
        //    needs_exchange:false,
        //},
        //StockSource {
        //    id: SourceID::CNBC,
        //    base_url: "https://www.cnbc.com/quotes/".to_string(),
        //    url_transformer: Box::new(|base_url, stock| Ok(format!("{}{}", base_url,stock.ticker.to_lowercase()))),
        //    price_result_transformer: Box::new(|s| s.replace("$", "")),
        //    price_selectors: vec![
        //        "div.QuoteStrip-dataContainer.QuoteStrip-extendedHours > div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice",
        //        "div.QuoteStrip-lastTimeAndPriceContainer > div.QuoteStrip-lastPriceStripContainer > span.QuoteStrip-lastPrice",
        //    ],
        //    needs_exchange:false,
        //},
        //StockSource {
        //    id: SourceID::WallStreetJournal,
        //    base_url: "https://www.wsj.com/market-data/quotes/".to_string(),
        //    url_transformer: Box::new(|base_url, stock| Ok(format!("{}{}", base_url,stock.ticker.to_uppercase()))),
        //    price_result_transformer: Box::new(|s| s.replace("$", "")),
        //    price_selectors: vec![
        //        "span#quote_val",
        //    ],
        //    needs_exchange:false,
        //},
    ];

    let stocks = vec![
        Stock::new("ROIV","NASDAQ"),
        Stock::new("GOOG","NASDAQ"),
        // Stock::new("MSFT","NASDAQ"),
        // Stock::new("SUM","NYSE"),
        // Stock::new("SHG","NYSE"),
        // Stock::new("DNPUF","OTCMKTS"),
    ];

    let mut rng = rand::thread_rng();
    for s in &stocks {
        println!("{}: {}",s.ticker,get_price_robust(s,&sources).unwrap());
    }
}
