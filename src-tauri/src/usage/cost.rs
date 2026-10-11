use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub output: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    pub model: String,
    pub currency: String,
    pub input: Option<String>,
    pub cache_read: Option<String>,
    pub cache_write: Option<String>,
    pub output: Option<String>,
    pub effective_at: i64,
}
pub fn decimal(value: &str, precision: usize) -> Result<i128, String> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() > 2
        || parts[0].is_empty()
        || !parts[0].bytes().all(|b| b.is_ascii_digit())
        || parts[0].len() > 12
    {
        return Err("单价必须为非负十进制数".into());
    }
    let fraction = parts.get(1).copied().unwrap_or("");
    if fraction.len() > precision || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("最多支持 {precision} 位小数"));
    }
    let joined = format!(
        "{}{}{}",
        parts[0],
        fraction,
        "0".repeat(precision - fraction.len())
    );
    joined.parse().map_err(|_| "单价值过大".into())
}
pub fn format_cost(value: i128) -> String {
    let scale = 1_000_000_000_000i128;
    format!("{}.{:012}", value / scale, value % scale)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
impl Price {
    pub fn validate(&self) -> Result<(), String> {
        if self.model.trim().is_empty()
            || self.currency.len() != 3
            || !self.currency.bytes().all(|b| b.is_ascii_uppercase())
        {
            return Err("请填写模型 ID 和三位大写币种代码".into());
        }
        for value in [
            &self.input,
            &self.cache_read,
            &self.cache_write,
            &self.output,
        ]
        .into_iter()
        .flatten()
        {
            decimal(value, 6)?;
        }
        Ok(())
    }
}
pub fn estimate(tokens: &Tokens, price: &Price) -> Result<Option<String>, String> {
    price.validate()?;
    let mut sum = 0i128;
    for (count, unit) in [
        (tokens.input, &price.input),
        (tokens.cache_read, &price.cache_read),
        (tokens.cache_write, &price.cache_write),
        (tokens.output, &price.output),
    ] {
        let Some(count) = count else { return Ok(None) };
        if count == 0 {
            continue;
        }
        let Some(unit) = unit else { return Ok(None) };
        sum = sum
            .checked_add(
                i128::from(count)
                    .checked_mul(decimal(unit, 6)?)
                    .ok_or("费用计算溢出")?,
            )
            .ok_or("费用计算溢出")?;
    }
    Ok(Some(format_cost(sum)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_cost_unknown_stays_unknown_and_decimal_math_is_exact() {
        let p = Price {
            model: "m".into(),
            currency: "USD".into(),
            input: Some("1.25".into()),
            cache_read: Some("0.1".into()),
            cache_write: Some("2.5".into()),
            output: Some("5".into()),
            effective_at: 0,
        };
        let t = Tokens {
            input: Some(1000000),
            cache_read: Some(1000000),
            cache_write: Some(1000000),
            output: Some(1000000),
        };
        assert_eq!(estimate(&t, &p).unwrap(), Some("8.85".into()));
        assert!(estimate(&Tokens::default(), &p).unwrap().is_none());
        let mut p = p;
        p.output = None;
        assert!(estimate(&t, &p).unwrap().is_none());
        assert!(decimal("-1", 6).is_err());
        assert!(decimal("NaN", 6).is_err());
    }
}
