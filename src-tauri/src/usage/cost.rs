use serde::{Deserialize, Serialize};

/// The five mutually exclusive counters Droid reports. `None` means unknown and
/// is never treated as zero.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub output: Option<u64>,
    #[serde(default)]
    pub thinking: Option<u64>,
}

impl Tokens {
    pub fn total(&self) -> Option<u64> {
        let mut sum = 0u64;
        for value in [
            self.input,
            self.cache_read,
            self.cache_write,
            self.output,
            self.thinking,
        ] {
            sum = sum.checked_add(value?)?;
        }
        Some(sum)
    }
    pub fn known_any(&self) -> bool {
        [
            self.input,
            self.cache_read,
            self.cache_write,
            self.output,
            self.thinking,
        ]
        .iter()
        .any(Option::is_some)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    /// Empty means the user-authored generic fallback for this API model.
    #[serde(default)]
    pub provider_id: String,
    pub model: String,
    pub currency: String,
    pub input: Option<String>,
    pub cache_read: Option<String>,
    pub cache_write: Option<String>,
    pub output: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    pub effective_at: i64,
}

pub struct Estimate {
    /// `None` when some counter is unknown, because a total would be fabricated.
    pub cost: Option<String>,
    /// False when any non-zero counter had no unit price, so the sum is a floor.
    pub complete: bool,
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

/// The inverse of [`format_cost`], for exact aggregation in SQL.
pub fn parse_cost(value: &str) -> Option<i128> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 12
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole: i128 = whole.parse().ok()?;
    let fraction: i128 = match fraction.is_empty() {
        true => 0,
        false => format!("{fraction:0<12}").parse().ok()?,
    };
    whole.checked_mul(1_000_000_000_000)?.checked_add(fraction)
}

impl Price {
    pub fn validate(&self) -> Result<(), String> {
        if self.model.trim().is_empty()
            || self.currency.len() != 3
            || !self.currency.bytes().all(|b| b.is_ascii_uppercase())
        {
            return Err("请填写模型 ID 和三位大写币种代码".into());
        }
        if !self.provider_id.trim().is_empty() && self.provider_id.len() > 128 {
            return Err("供应商标识过长".into());
        }
        for value in [
            &self.input,
            &self.cache_read,
            &self.cache_write,
            &self.output,
            &self.thinking,
        ]
        .into_iter()
        .flatten()
        {
            decimal(value, 6)?;
        }
        Ok(())
    }
}

/// A missing unit is only a problem when the counter is non-zero; a zero counter
/// needs no price.
pub fn estimate(tokens: &Tokens, price: &Price) -> Result<Estimate, String> {
    price.validate()?;
    let mut sum = 0i128;
    let mut complete = true;
    for (label, count, unit) in [
        ("input", tokens.input, &price.input),
        ("cacheRead", tokens.cache_read, &price.cache_read),
        ("cacheWrite", tokens.cache_write, &price.cache_write),
        ("output", tokens.output, &price.output),
        ("thinking", tokens.thinking, &price.thinking),
    ] {
        let Some(count) = count else {
            return Ok(Estimate {
                cost: None,
                complete: false,
            });
        };
        if count == 0 {
            continue;
        }
        let Some(unit) = unit else {
            complete = false;
            continue;
        };
        let _ = label;
        sum = sum
            .checked_add(
                i128::from(count)
                    .checked_mul(decimal(unit, 6)?)
                    .ok_or("费用计算溢出")?,
            )
            .ok_or("费用计算溢出")?;
    }
    Ok(Estimate {
        cost: Some(format_cost(sum)),
        complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn price() -> Price {
        Price {
            provider_id: String::new(),
            model: "m".into(),
            currency: "USD".into(),
            input: Some("1.25".into()),
            cache_read: Some("0.1".into()),
            cache_write: Some("2.5".into()),
            output: Some("5".into()),
            thinking: Some("5".into()),
            effective_at: 0,
        }
    }
    fn all(count: u64) -> Tokens {
        Tokens {
            input: Some(count),
            cache_read: Some(count),
            cache_write: Some(count),
            output: Some(count),
            thinking: Some(count),
        }
    }

    #[test]
    fn usage_cost_unknown_stays_unknown_and_decimal_math_is_exact() {
        assert_eq!(
            estimate(&all(1000000), &price()).unwrap().cost.unwrap(),
            "13.85"
        );
        assert!(estimate(&Tokens::default(), &price())
            .unwrap()
            .cost
            .is_none());
        let mut p = price();
        p.output = None;
        let partial = estimate(&all(1000000), &p).unwrap();
        assert!(!partial.complete);
        assert_eq!(partial.cost.unwrap(), "8.85");
        assert!(decimal("-1", 6).is_err());
        assert!(decimal("NaN", 6).is_err());
    }

    #[test]
    fn zero_counters_need_no_price_and_thinking_is_priced_separately() {
        let mut p = price();
        p.thinking = None;
        let only_input = Tokens {
            input: Some(1000000),
            cache_read: Some(0),
            cache_write: Some(0),
            output: Some(0),
            thinking: Some(0),
        };
        let only_input_estimate = estimate(&only_input, &p).unwrap();
        assert!(only_input_estimate.complete);
        assert_eq!(only_input_estimate.cost.unwrap(), "1.25");
        let with_thinking = Tokens {
            thinking: Some(1000000),
            ..only_input
        };
        let thinking_estimate = estimate(&with_thinking, &p).unwrap();
        assert!(!thinking_estimate.complete);
        assert_eq!(thinking_estimate.cost.unwrap(), "1.25");
        assert_eq!(all(1).total(), Some(5));
        assert_eq!(Tokens::default().total(), None);
        assert!(!Tokens::default().known_any());
    }
}
