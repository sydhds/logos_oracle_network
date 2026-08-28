use crate::BinancePriceEvent;

use rust_decimal::Decimal;
use std::str::FromStr;
const PRICE_OBSERVATION_DECIMAL: u32 = 6;

#[derive(Debug, Clone)]
pub struct PartialPriceObservation {
    feed_id: String,
    price: i64,
    decimals: i32,
    timestamp: i64,
}

impl TryFrom<(String, BinancePriceEvent)> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from((feed_id_normalized, event): (String, BinancePriceEvent)) -> Result<Self, Self::Error> {

        let mut decimal = Decimal::from_str(event.last_price.as_str()).expect("Failed to parse Binance price");
        decimal.rescale(PRICE_OBSERVATION_DECIMAL);
        let scaled_i64 = decimal.mantissa() as i64;
        Ok(Self {
            feed_id: feed_id_normalized,
            price: scaled_i64,
            decimals: PRICE_OBSERVATION_DECIMAL as i32,
            timestamp: event.event_time.try_into()?,
        })
    }
}