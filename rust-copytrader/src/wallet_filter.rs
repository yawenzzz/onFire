#[derive(Debug, Clone, PartialEq)]
pub struct ActivityRecord {
    pub wallet: String,
    pub timestamp: u64,
    pub event_type: String,
    pub size: f64,
    pub usdc_size: f64,
    pub transaction_hash: Option<String>,
    pub price: Option<f64>,
    pub asset: String,
    pub side: Option<String>,
    pub condition_id: Option<String>,
    pub outcome: Option<String>,
    pub slug: Option<String>,
}

pub fn parse_activity_records(content: &str) -> Vec<ActivityRecord> {
    json_objects(content)
        .into_iter()
        .filter_map(|object| {
            let wallet = extract_json_field(&object, "proxyWallet")
                .or_else(|| extract_json_field(&object, "user"))
                .unwrap_or_default();
            let timestamp =
                extract_json_field(&object, "timestamp").and_then(|value| parse_u64(&value))?;
            let event_type =
                extract_json_field(&object, "type").unwrap_or_else(|| "TRADE".to_string());
            let asset = extract_json_field(&object, "asset").unwrap_or_default();
            Some(ActivityRecord {
                wallet,
                timestamp,
                event_type,
                size: extract_json_field(&object, "size")
                    .and_then(|value| parse_f64(&value))
                    .unwrap_or(0.0),
                usdc_size: extract_json_field(&object, "usdcSize")
                    .and_then(|value| parse_f64(&value))
                    .unwrap_or(0.0),
                transaction_hash: extract_json_field(&object, "transactionHash"),
                price: extract_json_field(&object, "price").and_then(|value| parse_f64(&value)),
                asset,
                side: extract_json_field(&object, "side"),
                condition_id: extract_json_field(&object, "conditionId"),
                outcome: extract_json_field(&object, "outcome"),
                slug: extract_json_field(&object, "slug"),
            })
        })
        .collect()
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

fn json_objects(content: &str) -> Vec<String> {
    let mut objects = Vec::new();
    let mut cursor = 0usize;
    while let Some(offset) = content[cursor..].find('{') {
        let start = cursor + offset;
        if let Some((from, to)) = object_bounds(content, start) {
            objects.push(content[from..=to].to_string());
            cursor = to + 1;
        } else {
            break;
        }
    }
    objects
}

fn object_bounds(content: &str, anchor: usize) -> Option<(usize, usize)> {
    let bytes = content.as_bytes();
    let start = content[..=anchor].rfind('{')?;
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, byte) in bytes.iter().enumerate().skip(start) {
        match byte {
            b'\\' if in_string && !escaped => {
                escaped = true;
                continue;
            }
            b'"' if !escaped => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, idx));
                }
            }
            _ => {}
        }
        escaped = false;
    }
    None
}

fn extract_json_field(object: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":");
    let start = object.find(&needle)?;
    let rest = object[start + needle.len()..].trim_start();
    if let Some(rest) = rest.strip_prefix('"') {
        let mut escaped = false;
        for (idx, ch) in rest.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => return Some(rest[..idx].to_string()),
                _ => {}
            }
        }
        None
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{ActivityRecord, parse_activity_records};

    #[test]
    fn parse_activity_records_extracts_trade_fields() {
        let records = parse_activity_records(
            r#"[{"proxyWallet":"0xabc","timestamp":123,"type":"TRADE","size":12.5,"usdcSize":10.0,"transactionHash":"0xtx","price":0.8,"asset":"asset-1","side":"BUY","conditionId":"cond-1","outcome":"No","slug":"slug-1"}]"#,
        );

        assert_eq!(
            records,
            vec![ActivityRecord {
                wallet: "0xabc".into(),
                timestamp: 123,
                event_type: "TRADE".into(),
                size: 12.5,
                usdc_size: 10.0,
                transaction_hash: Some("0xtx".into()),
                price: Some(0.8),
                asset: "asset-1".into(),
                side: Some("BUY".into()),
                condition_id: Some("cond-1".into()),
                outcome: Some("No".into()),
                slug: Some("slug-1".into()),
            }]
        );
    }
}
