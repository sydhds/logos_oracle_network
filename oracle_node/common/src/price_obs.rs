use crate::BinancePriceEvent;

use rust_decimal::Decimal;
use std::str::FromStr;
use anyhow::anyhow;
use rust_decimal::prelude::FromPrimitive;
use crate::redstone::RedstonePriceEvent;

const PRICE_OBSERVATION_DECIMAL: u32 = 6;

#[derive(Debug, Clone)]
pub struct PartialPriceObservation {
    pub feed_id: String,
    pub feed_id_provider: String,
    pub price: i64,
    pub decimals: i32,
    pub timestamp: i64,
}

impl TryFrom<(String, BinancePriceEvent)> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from((feed_id_normalized, event): (String, BinancePriceEvent)) -> Result<Self, Self::Error> {

        let mut decimal = Decimal::from_str(event.last_price.as_str())?;
        decimal.rescale(PRICE_OBSERVATION_DECIMAL);
        let scaled_i64 = decimal.mantissa() as i64;
        Ok(Self {
            feed_id: feed_id_normalized,
            feed_id_provider: event.symbol,
            price: scaled_i64,
            decimals: PRICE_OBSERVATION_DECIMAL as i32,
            timestamp: event.event_time.try_into()?,
        })
    }
}

impl TryFrom<(String, RedstonePriceEvent)> for PartialPriceObservation {
    type Error = anyhow::Error;
    fn try_from((feed_id_normalized, event): (String, RedstonePriceEvent)) -> Result<Self, Self::Error> {

        let mut decimal = Decimal::from_f64(event.value)
            .ok_or(anyhow!("Failed to parse Redstone price value"))?;

        decimal.rescale(PRICE_OBSERVATION_DECIMAL);
        let scaled_i64 = decimal.mantissa() as i64;
        Ok(Self {
            feed_id: feed_id_normalized,
            feed_id_provider: event.symbol,
            price: scaled_i64,
            decimals: PRICE_OBSERVATION_DECIMAL as i32,
            timestamp: event.timestamp.try_into()?,
        })
    }
}