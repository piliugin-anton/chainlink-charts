//! Декодирование цен Chainlink для этого проекта.
//!
//! REST `history/rows` и поток тиков отдают OHLC/цену как большое число (практически fixed-point,
//! в коде веб-клиента — масштаб **1e18**), см. [`src/lib/chainlink/price.ts`](../../../../src/lib/chainlink/price.ts).
//! Обзор продукта: [Chainlink Data Streams](https://docs.chain.link/data-streams). В этом репозитории
//! числа в JSON истории и стрима обрабатываются так же, как в `src/lib/chainlink/price.ts`.

/// Тот же масштаб, что `PRICE_SCALE` в TypeScript.
pub const PRICE_SCALE: f64 = 1e18;

#[inline]
pub fn decode_chainlink_price(raw: f64) -> f64 {
    if !raw.is_finite() || raw == 0.0 {
        0.0
    } else {
        raw / PRICE_SCALE
    }
}

/// Обратно к сырому виду, как в строках API истории.
#[inline]
pub fn encode_chainlink_price(value: f64) -> f64 {
    if !value.is_finite() {
        0.0
    } else {
        value * PRICE_SCALE
    }
}
