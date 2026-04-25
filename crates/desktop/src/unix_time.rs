//! Форматирование Unix-времени (ось графика = секунды с 1970-01-00 UTC) без внешних крейтов.

const MON3: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn is_leap_y(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn year_len_days(y: i32) -> i64 {
    if is_leap_y(y) { 366 } else { 365 }
}

/// Календарь UTC: `days` — полных суток с 1970-01-01, день 0 = 1970-01-01.
fn civil_from_epoch_days(mut days: i64) -> (i32, u32, u32) {
    let mut y: i32 = 1970;
    loop {
        let yl = year_len_days(y);
        if days < yl {
            break;
        }
        days -= yl;
        y += 1;
    }
    const MD_LEAP: [i64; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    const MD: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let md = if is_leap_y(y) { MD_LEAP } else { MD };
    for (mi, &dlen) in md.iter().enumerate() {
        if days < dlen {
            return (y, (mi as u32) + 1, (days as u32) + 1);
        }
        days -= dlen;
    }
    (y, 12, 1)
}

fn hms_in_day(t: u64) -> (u32, u32, u32) {
    let h = (t / 3600) as u32;
    let m = ((t % 3600) / 60) as u32;
    let s = (t % 60) as u32;
    (h, m, s)
}

/// `YYYY-MM-DD  HH:MM:SS UTC` для `secs >= 0` (тики Chainlink / история BFF).
pub fn format_compact_utc(secs: i64) -> String {
    if secs < 0 {
        return format!("(invalid t: {secs})");
    }
    let u = secs as u64;
    let d = (u / 86_400) as i64;
    let t = u % 86_400;
    let (h, m, s) = hms_in_day(t);
    let (y, mo, ddom) = civil_from_epoch_days(d);
    format!(
        "{:04}-{:02}-{:02}  {:02}:{:02}:{:02} UTC",
        y, mo, ddom, h, m, s
    )
}

/// Подписи оси X (plot) по текущему зум-диапазону (ширина в тех же «секундах»).
pub fn format_axis_label_utc(secs: i64, range_width: f64) -> String {
    if secs < 0 {
        return format!("{secs}");
    }
    let u = secs as u64;
    let d = (u / 86_400) as i64;
    let t = u % 86_400;
    let (h, mi, _s) = hms_in_day(t);
    let (y, mo, ddom) = civil_from_epoch_days(d);
    let w = range_width.max(1.0);
    if w > 90.0 * 86400.0 {
        format!("{y:04}-{mo:02}")
    } else if w > 2.0 * 86400.0 {
        let mon = MON3[(mo as usize).saturating_sub(1) % 12];
        format!("{ddom} {mon}")
    } else if w > 4.0 * 3600.0 {
        let mon = MON3[(mo as usize).saturating_sub(1) % 12];
        format!("{ddom} {mon} {h:02}:{mi:02}")
    } else {
        format!("{h:02}:{mi:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_utc() {
        assert_eq!(
            format_compact_utc(0),
            "1970-01-01  00:00:00 UTC"
        );
    }

    #[test]
    fn one_day() {
        assert_eq!(
            format_compact_utc(86_400),
            "1970-01-02  00:00:00 UTC"
        );
    }
}
