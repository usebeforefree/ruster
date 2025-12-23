use http::StatusCode;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub(crate) struct StatusCodeRange {
    start: StatusCode,
    end: StatusCode,
}

impl StatusCodeRange {
    /// checks if the given code is inside the range (inclusive)
    pub(crate) fn contains(&self, code: StatusCode) -> bool {
        code >= self.start && code <= self.end
    }
}

pub(crate) fn is_code_in_ranges(code: StatusCode, ranges: &[StatusCodeRange]) -> bool {
    ranges.iter().any(|range| range.contains(code))
}

impl FromStr for StatusCodeRange {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((start, end)) = s.split_once('-') {
            let start: StatusCode = start.parse().map_err(|_| "Invalid number")?;
            let end: StatusCode = end.parse().map_err(|_| "Invalid number")?;
            if start > end {
                return Err("Range start > end".into());
            }
            Ok(StatusCodeRange { start, end })
        } else {
            let val: StatusCode = s.parse().map_err(|_| "Invalid number")?;
            Ok(StatusCodeRange {
                start: val,
                end: val,
            })
        }
    }
}
