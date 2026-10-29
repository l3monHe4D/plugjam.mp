//! Оплата подписки: USDT в сети TON на кошелёк проекта.
//! Ссылка по формату https://docs.ton.org/ecosystem/wallet-apps/deep-links
//! (`ton://transfer/<адрес>?jetton=<мастер USDT>&amount=<микро-USDT>&text=<комментарий>`).

use serde::Serialize;

pub const WALLET: &str = "UQDqf8maIbXABw9LoRsI5ruG7MBAY0wP6c3bUdsEJWc3pnBq";
pub const USDT_MASTER: &str = "EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs";
/// 0,99 USDT; у USDT 6 знаков после запятой.
pub const PRICE_MICRO: u64 = 990_000;
pub const PRICE_TEXT: &str = "0.99";
pub const PERIOD_DAYS: u64 = 30;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Donate {
    pub wallet: String,
    pub price: String,
    pub period_days: u64,
    /// Комментарий к платежу — ID пользователя; по нему владелец находит платёж.
    pub comment: String,
    pub link: String,
}

pub fn info(user_id: &str) -> Donate {
    Donate {
        wallet: WALLET.into(),
        price: PRICE_TEXT.into(),
        period_days: PERIOD_DAYS,
        comment: user_id.into(),
        link: format!("ton://transfer/{WALLET}?jetton={USDT_MASTER}&amount={PRICE_MICRO}&text={user_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_shape() {
        let d = info("PJ-ABCD-EFGH-JKMN");
        assert_eq!(
            d.link,
            "ton://transfer/UQDqf8maIbXABw9LoRsI5ruG7MBAY0wP6c3bUdsEJWc3pnBq?jetton=EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs&amount=990000&text=PJ-ABCD-EFGH-JKMN"
        );
        assert_eq!(d.price, "0.99");
    }
}
