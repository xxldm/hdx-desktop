use std::time::{SystemTime, UNIX_EPOCH};

const REFRESH_SKEW_SECONDS: u64 = 60;

pub(super) fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(super) fn should_refresh(access_token_expires_at: u64) -> bool {
    access_token_expires_at.saturating_sub(now_epoch_seconds()) <= REFRESH_SKEW_SECONDS
}

pub(super) fn parse_iso_timestamp(value: &str) -> Result<u64, String> {
    let trimmed = value.trim();
    // 后端返回 Instant.toString() 格式，固定以 Z 结尾（UTC）。
    if !trimmed.ends_with('Z') {
        return Err(format!("认证中心返回的时间格式无效：{value}"));
    }

    let datetime_part = &trimmed[..trimmed.len() - 1];
    let (date_part, time_part) = datetime_part
        .split_once('T')
        .ok_or_else(|| format!("认证中心返回的时间格式无效：{value}"))?;

    let (year, month, day) = parse_date(date_part)?;
    let (hour, minute, second) = parse_time(time_part)?;

    Ok(epoch_seconds(year, month, day, hour, minute, second))
}

fn parse_date(part: &str) -> Result<(u32, u32, u32), String> {
    let segments: Vec<&str> = part.split('-').collect();
    if segments.len() != 3 {
        return Err(format!("日期格式无效：{part}"));
    }
    let year = segments[0]
        .parse::<u32>()
        .map_err(|_| format!("年份无效：{part}"))?;
    let month = segments[1]
        .parse::<u32>()
        .map_err(|_| format!("月份无效：{part}"))?;
    let day = segments[2]
        .parse::<u32>()
        .map_err(|_| format!("日期无效：{part}"))?;
    Ok((year, month, day))
}

fn parse_time(part: &str) -> Result<(u32, u32, u32), String> {
    // 去掉可能的毫秒部分（如 12:00:00.123）。
    let main = part.split('.').next().unwrap_or(part);
    let segments: Vec<&str> = main.split(':').collect();
    if segments.len() < 2 || segments.len() > 3 {
        return Err(format!("时间格式无效：{part}"));
    }
    let hour = segments[0]
        .parse::<u32>()
        .map_err(|_| format!("小时无效：{part}"))?;
    let minute = segments[1]
        .parse::<u32>()
        .map_err(|_| format!("分钟无效：{part}"))?;
    let second = if segments.len() == 3 {
        segments[2]
            .parse::<u32>()
            .map_err(|_| format!("秒无效：{part}"))?
    } else {
        0
    };
    Ok((hour, minute, second))
}

/// 公历 UTC epoch 秒（Howard Hinnant 算法，不处理闰秒，精度对 token 过期判断足够）。
fn epoch_seconds(year: u32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> u64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let m = month as i64;
    let d = day as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    (era * 146097 + doe as i64 - 719468) as u64 * 86400
        + hour as u64 * 3600
        + minute as u64 * 60
        + second as u64
}
