/// Reads a fixed-width table: a header, a line of dashes, then rows. Columns start where the
/// dash groups start (scoop) or, when the dashes are one solid line, where header words start
/// (winget). Positions work whatever language the header is in.
pub fn parse(text: &str) -> Vec<Vec<String>> {
    // winget redraws a spinner with \r before the table, keep only what was drawn last.
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.rsplit('\r').next().unwrap_or(l))
        .collect();
    let Some(sep) = lines.iter().position(|l| {
        let t = l.trim();
        t.starts_with("--") && t.chars().all(|c| c == '-' || c == ' ')
    }) else {
        return Vec::new();
    };
    let Some(header) = sep.checked_sub(1).map(|i| lines[i]) else {
        return Vec::new();
    };

    let starts_of = |line: &str| -> Vec<usize> {
        let chars: Vec<char> = line.chars().collect();
        (0..chars.len())
            .filter(|&i| chars[i] != ' ' && (i == 0 || chars[i - 1] == ' '))
            .collect()
    };
    let dash_starts = starts_of(lines[sep]);
    let starts = if dash_starts.len() > 1 {
        dash_starts
    } else {
        starts_of(header)
    };

    lines[sep + 1..]
        .iter()
        .take_while(|l| !l.trim().is_empty())
        .map(|row| {
            let chars: Vec<char> = row.chars().collect();
            starts
                .iter()
                .enumerate()
                .map(|(k, &start)| {
                    let end = starts.get(k + 1).copied().unwrap_or(chars.len());
                    chars
                        .get(start..end.min(chars.len()))
                        .map(|cell| cell.iter().collect::<String>().trim().to_string())
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_header_words_under_a_solid_line() {
        let text = "  \r-\r\\\rName            Id                  Version  Available Source\n------------------------------------------------------------------\nGit             Git.Git             2.46.0   2.47.0    winget\nVisual Studio … Microsoft.VisualStu 17.11.4  17.11.5   winget\n2 upgrades available.\n";
        let rows = parse(text);
        assert_eq!(rows[0], ["Git", "Git.Git", "2.46.0", "2.47.0", "winget"]);
        assert_eq!(rows[1][1], "Microsoft.VisualStu");
        // The summary line becomes a row with no versions, which callers drop.
        assert_eq!(rows[2][3], "");
    }

    #[test]
    fn uses_dash_groups_for_multi_word_headers() {
        let text = "Name    Installed Version Latest Version Missing Dependencies Info\n----    ----------------- -------------- -------------------- ----\ngit     2.46.0            2.47.0\n";
        let rows = parse(text);
        assert_eq!(rows[0][..3], ["git", "2.46.0", "2.47.0"]);
    }

    #[test]
    fn no_table_means_no_rows() {
        assert!(parse("Scoop is up to date.\n").is_empty());
    }
}
