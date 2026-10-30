//! Получение подписки: USDT в сети TON на кошелёк проекта или обращение к разработчику в Telegram.

use serde::Serialize;

pub const WALLET: &str = "UQDqf8maIbXABw9LoRsI5ruG7MBAY0wP6c3bUdsEJWc3pnBq";
/// 0,99 USDT; у USDT 6 знаков после запятой.
pub const PRICE_TEXT: &str = "0.99";
pub const PERIOD_DAYS: u64 = 30;
pub const TELEGRAM_HANDLE: &str = "@irishatemskey";
pub const TELEGRAM_URL: &str = "https://t.me/irishatemskey";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Donate {
    pub wallet: String,
    pub price: String,
    pub period_days: u64,
    /// Комментарий к платежу — ID пользователя; по нему владелец находит платёж.
    pub comment: String,
    pub telegram_handle: String,
    pub telegram_url: String,
}

pub fn info(user_id: &str) -> Donate {
    Donate {
        wallet: WALLET.into(),
        price: PRICE_TEXT.into(),
        period_days: PERIOD_DAYS,
        comment: user_id.into(),
        telegram_handle: TELEGRAM_HANDLE.into(),
        telegram_url: TELEGRAM_URL.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_fields() {
        let d = info("PJ-ABCD-EFGH-JKMN");
        assert_eq!(d.wallet, "UQDqf8maIbXABw9LoRsI5ruG7MBAY0wP6c3bUdsEJWc3pnBq");
        assert_eq!(d.comment, "PJ-ABCD-EFGH-JKMN");
        assert_eq!(d.price, "0.99");
        assert_eq!(d.telegram_url, "https://t.me/irishatemskey");
        assert!(TELEGRAM_URL.ends_with(&TELEGRAM_HANDLE[1..]));
    }
}
