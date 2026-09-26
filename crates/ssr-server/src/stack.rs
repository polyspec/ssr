use sourcemap::SourceMap;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) enum Error {
    InvalidFrame(String),
    MissingMapping(String, u32, u32),
    MissingSource(String, u32, u32),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFrame(frame) => write!(f, "invalid generated stack frame: {frame}"),
            Self::MissingMapping(path, line, column) => {
                write!(f, "source map has no entry for {path}:{line}:{column}")
            }
            Self::MissingSource(path, line, column) => {
                write!(
                    f,
                    "source map entry has no source for {path}:{line}:{column}"
                )
            }
        }
    }
}

pub(crate) fn map_stack(stack: &str, maps: &BTreeMap<String, SourceMap>) -> Result<String, Error> {
    let mut output = String::new();
    for (index, frame) in stack.lines().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        let Some((start, path, map)) = maps
            .iter()
            .filter_map(|(path, map)| {
                let needle = format!("{path}:");
                frame
                    .rfind(&needle)
                    .filter(|start| {
                        *start == 0 || matches!(frame.as_bytes()[start - 1], b'(' | b' ')
                    })
                    .map(|start| (start, path, map))
            })
            .max_by_key(|(start, path, _)| (*start, path.len()))
        else {
            if frame.trim_start().starts_with("at ") && frame.contains("server/") {
                return Err(Error::InvalidFrame(frame.into()));
            }
            output.push_str(frame);
            continue;
        };
        let coordinates = &frame[start + path.len() + 1..];
        let (line, column_with_suffix) = coordinates
            .split_once(':')
            .ok_or_else(|| Error::InvalidFrame(frame.into()))?;
        let column = column_with_suffix.trim_end_matches(')');
        let generated_line: u32 = line
            .parse()
            .map_err(|_| Error::InvalidFrame(frame.into()))?;
        let generated_column: u32 = column
            .parse()
            .map_err(|_| Error::InvalidFrame(frame.into()))?;
        if generated_line == 0 || generated_column == 0 {
            return Err(Error::InvalidFrame(frame.into()));
        }
        let token = map
            .lookup_token(generated_line - 1, generated_column - 1)
            .ok_or_else(|| Error::MissingMapping(path.clone(), generated_line, generated_column))?;
        let source = token
            .get_source()
            .ok_or_else(|| Error::MissingSource(path.clone(), generated_line, generated_column))?;
        output.push_str(&frame[..start]);
        output.push_str(source);
        output.push(':');
        output.push_str(&(token.get_src_line() + 1).to_string());
        output.push(':');
        output.push_str(&(token.get_src_col() + 1).to_string());
        output.push_str(&column_with_suffix[column.len()..]);
    }
    if stack.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sourcemap::SourceMapBuilder;

    #[test]
    fn maps_generated_frame_and_keeps_unrelated_frame() {
        let mut builder = SourceMapBuilder::new(Some("server/entry.js"));
        builder.add(0, 4, 9, 2, Some("src/page.tsx"), None, false);
        let map = builder.into_sourcemap();
        let maps = BTreeMap::from([("server/entry.js".into(), map)]);
        assert_eq!(
            map_stack(
                "Error: failed\n    at render (server/entry.js:1:5)\n    at other (state.js:1:2)\n    at similar (other-server.js:1:5)",
                &maps
            )
            .unwrap(),
            "Error: failed\n    at render (src/page.tsx:10:3)\n    at other (state.js:1:2)\n    at similar (other-server.js:1:5)"
        );
        assert!(map_stack("at render (server/entry.js:0:5)", &maps).is_err());
        assert!(map_stack("at render (server/entry.js:1:1)", &maps).is_err());
        assert!(map_stack("at render (server/unknown.js:1:1)", &maps).is_err());
    }

    #[test]
    fn maps_each_server_chunk_with_its_own_source_map() {
        let mut entry = SourceMapBuilder::new(Some("server/entry-123.js"));
        entry.add(0, 0, 2, 1, Some("src/entry.ts"), None, false);
        let mut chunk = SourceMapBuilder::new(Some("server/chunk-456.js"));
        chunk.add(0, 0, 7, 3, Some("src/chunk.ts"), None, false);
        let maps = BTreeMap::from([
            ("server/entry-123.js".into(), entry.into_sourcemap()),
            ("server/chunk-456.js".into(), chunk.into_sourcemap()),
        ]);
        assert_eq!(
            map_stack(
                "Error: failed\n    at render (server/entry-123.js:1:1)\n    at part (server/chunk-456.js:1:1)",
                &maps,
            )
            .unwrap(),
            "Error: failed\n    at render (src/entry.ts:3:2)\n    at part (src/chunk.ts:8:4)"
        );
    }
}
