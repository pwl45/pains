extern crate rand;
extern crate regex;
use std::fmt::Debug;
use thiserror::Error;
use reqwest::header;
use rand::seq::SliceRandom;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use strum_macros::Display;
use std::collections::HashMap;
extern crate maplit;
use maplit::hashmap;

use regex::Regex;

struct StockSource {
    id: SourceID,
    base_url: String,
    url_transformer: fn(&str, &Stock) -> Result<String, StockScrapeError>,
    attributes: HashMap<AttrID, AttrGettr>,
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

#[derive(Debug, EnumIter, Display, Eq, Hash, PartialEq, Clone, Copy)]
enum AttrID {
    Price,
    PctCh,
    PE,
}

struct AttrGettr {
    attr_id: AttrID,
    get_selectors: fn(&Stock) -> Vec<String>,
    result_transformer: fn(&str) -> String,
}

impl std::fmt::Display for AttrGettr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.attr_id)
    }
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

    #[error("No matching attribute (AttrID)")]
    NoAttrError,
}

#[derive(Debug)]
pub enum AggregatedError {
    AllFailed(Vec<StockScrapeError>),
}

fn coalesce_maps<K, V, E>(
    dest: &mut HashMap<K, Result<V, E>>,
    src: HashMap<K, Result<V, E>>,
) where
K: std::cmp::Eq + std::hash::Hash + Clone,
    // V: Clone,
    // E: Clone,
{
    for (key, val) in src {
        match dest.get(&key) {
            Some(Ok(_)) => {
                if let Err(_) = &val {
                    // dest[key] is Ok and src[key] is Err, so do not insert
                    continue;
                }
            }
            _ => {}
        }
        // Insert (key, val) into dest
        dest.insert(key, val);
    }
}

fn get_document(url:&str) -> Result<scraper::Html,StockScrapeError>{
    let client = reqwest::blocking::Client::new();
    let response=client.get(url).header(header::USER_AGENT,"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/109.0.0.0 Safari/537.36").send()
        .map_err(StockScrapeError::FetchError)?
        .text()
        .map_err(StockScrapeError::ReadError)?;
    return Ok(scraper::Html::parse_document(&response));
}


fn get_attr_multiple_selector<F>(
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

fn get_all_attrs_robust(s: &Stock, sources: &Vec<StockSource>) -> HashMap<AttrID, Result<String, StockScrapeError>> {
    let all_attrs = AttrID::iter().collect();
    return get_attrs_robust(s,sources,&all_attrs);
}

fn get_attrs_robust(s: &Stock, sources: &Vec<StockSource>, attrs: &Vec<AttrID>) 
    -> HashMap<AttrID, Result<String, StockScrapeError>> {

        let mut agg_results: HashMap<AttrID, Result<String, StockScrapeError>> = attrs.iter().cloned()
            .map(|attr| (attr, Err(StockScrapeError::NoMatchError)))
            .collect();
        // do random
        let mut indices: Vec<usize> = (0..sources.len()).collect();
        indices.shuffle(&mut rand::thread_rng());
        for i in indices{
            //Check if every key in attrs maps to an OK value in agg_results. If so, return agg_results. Otherwise, continue
            if attrs.iter().all(|attr| {
                match agg_results.get(attr) {
                    Some(Ok(_)) => true,
                    _ => false,
                }
            }) {
                return agg_results;
            }
            let source = &sources[i];
            match get_attrs_source(s, source, attrs) {
                Ok(source_map) => {
                    coalesce_maps(&mut agg_results,source_map);
                },
                Err(_) => continue,
            }
            // println!("{:?}",agg_results);
        }
        return agg_results;
}


fn get_attrs_source(s: &Stock, so: &StockSource, attrs: &Vec<AttrID>) -> Result<HashMap<AttrID, Result<String, StockScrapeError>>, StockScrapeError> {
    let full_url = (so.url_transformer)(&so.base_url, &s)?;
    // println!("{}",full_url);
    let document = get_document(&full_url)?;

    let mut results = HashMap::new();

    for attr_id in attrs {
        if let Some(attr) = so.attributes.get(attr_id) {
            let selectors = (attr.get_selectors)(s);
            let result = get_attr_multiple_selector(&document, &selectors, &attr.result_transformer);
            results.insert(*attr_id, result);
        }else{
            results.insert(*attr_id,Err(StockScrapeError::NoAttrError));
        }
    }

    Ok(results)
}
fn test_stonk(sources: &Vec<StockSource>){
    let stocks = vec![
        Stock::new("ROIV","NASDAQ"),
        Stock::new("GOOG","NASDAQ"),
        Stock::new("MSFT","NASDAQ"),
        Stock::new("SUM","NYSE"),
        Stock::new("SHG","NYSE"),
        Stock::new("DNPUF","OTCMKTS"),
    ];

    for s in &stocks {
        let result_dict=get_all_attrs_robust(s,sources);
        println!("{:?}",&result_dict);
    }
}

fn get_quote(s: &Stock, sources: &Vec<StockSource>)-> String{
    let attrs = vec![AttrID::Price, AttrID::PctCh];
    let result_dict: HashMap<AttrID, Result<String, StockScrapeError>>  = get_attrs_robust(s,sources,&attrs);
    let price = match result_dict.get(&AttrID::Price) {
        Some(Ok(price)) => price.clone(),
        Some(Err(_)) => "?".to_string(),
        None => "?".to_string(),
    };
    let pctch = match result_dict.get(&AttrID::PctCh) {
        Some(Ok(price)) => price.clone(),
        Some(Err(_)) => "?".to_string(),
        None => "?".to_string(),
    };
    return format!("{}: {} ({})",s.ticker,price,pctch);
}

fn main() {
    let sources = vec![
        StockSource {
            id: SourceID::Yahoo,
            base_url: "https://finance.yahoo.com/quote/".to_string(),
            url_transformer: |base_url, stock| Ok(format!("{}{}/", base_url, stock.ticker)),
            attributes: hashmap![
                AttrID::Price => AttrGettr {
                    attr_id: AttrID::Price,
                    get_selectors: |_stock: &Stock| {
                        vec!["fin-streamer.Fw\\(b\\).Fz\\(36px\\).Mb\\(-4px\\).D\\(ib\\)".to_string()]
                    },
                    result_transformer: |result: &str| {
                        result.replace("$","")
                    },
                },
                AttrID::PctCh => AttrGettr {
                    attr_id: AttrID::PctCh,
                    get_selectors: |stock: &Stock| {
                        let selector = format!(r#"fin-streamer[data-field="regularMarketChangePercent"][data-symbol="{}"]"#,stock.ticker);
                        vec![selector]
                    },
                    result_transformer: |result: &str| {
                        let re = Regex::new(r".*\((.*)\)").unwrap();
                        if let Some(captured) = re.captures(result) {
                            if let Some(matched) = captured.get(1) {
                                return matched.as_str().to_string();
                            }
                        }
                        return "".to_string();
                    },
                },
                ],
                needs_exchange: false,
        },
        StockSource {
            id: SourceID::SeekingAlpha,
            base_url: "https://seekingalpha.com/symbol/".to_string(),
            url_transformer: |base_url, stock| Ok(format!("{}{}/", base_url, stock.ticker)),
            attributes: hashmap![
                AttrID::Price => AttrGettr {
                    attr_id: AttrID::Price,
                    get_selectors: |_stock: &Stock| {
                        vec![r#"[data-test-id="symbol-price"]"#.to_string()]
                    },
                    result_transformer: |result: &str| {
                        result.replace("$","")
                    },
                },
                AttrID::PctCh => AttrGettr {
                    attr_id: AttrID::PctCh,
                    get_selectors: |_stock: &Stock| {
                        vec![r#"[data-test-id="symbol-change"]"#.to_string()]
                    },
                    result_transformer: |result: &str| {
                        let re = Regex::new(r".*\((.*)\)").unwrap();
                        if let Some(captured) = re.captures(result) {
                            if let Some(matched) = captured.get(1) {
                                return matched.as_str().to_string();
                            }
                        }
                        return "".to_string();
                    },
                },
                ],
                needs_exchange: false,
        },
        StockSource {
            id: SourceID::GoogleFinance,
           base_url: "https://www.google.com/finance/quote/".to_string(),
           url_transformer: |base_url, stock| {
               let exchange_str= stock.exchange.as_ref().ok_or(StockScrapeError::NoExchangeError)?;
               Ok(format!("{}{}:{}", base_url, stock.ticker,exchange_str))
           },
            attributes: hashmap![
                AttrID::Price => AttrGettr {
                    attr_id: AttrID::Price,
                    get_selectors: |_stock: &Stock| {
                        vec!["div.YMlKec.fxKbKc".to_string()]
                    },
                    result_transformer: |result: &str| {
                        result.replace("$","")
                    },
                },
                AttrID::PctCh => AttrGettr {
                    attr_id: AttrID::PctCh,
                    get_selectors: |_stock: &Stock| {
                        vec![r#"div.yWOrNb span[jsname="Fe7oBc"].NydbP div.JwB6zf"#.to_string()]
                    },
                    result_transformer: |result: &str| {
                        return result.to_string();
                    },
                },
                ],
                needs_exchange: false,
        },
        ];

    // test_stonk(&sources);
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        for arg in &args[1..]{
            let parts: Vec<&str> = arg.split(':').collect();
            let ticker = parts[0].trim().to_string();
            let exchange = parts.get(1).map(|s| s.trim().to_string());
            println!("{}",get_quote(&Stock{ticker: ticker,exchange: exchange},&sources));
        }
    } else {
        println!("No arguments were provided");
        std::process::exit(1);
    }


}

