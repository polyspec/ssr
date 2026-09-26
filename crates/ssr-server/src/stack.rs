use sourcemap::SourceMap;

#[derive(Debug)]
pub(crate) enum Error {
    InvalidFrame(String),
    MissingMapping(u32, u32),
    MissingSource(u32, u32),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFrame(frame) => write!(f, "invalid generated stack frame: {frame}"),
            Self::MissingMapping(line, column) => {
                write!(f, "source map has no entry for server.js:{line}:{column}")
            }
            Self::MissingSource(line, column) => {
                write!(
                    f,
                    "source map entry has no source for server.js:{line}:{column}"
                )
            }
        }
    }
}

pub(crate) fn map_stack(stack: &str, map: &SourceMap) -> Result<String, Error> {
    let mut output = String::new();
    for (index, frame) in stack.lines().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        let Some(start) = frame
            .rfind("server.js:")
            .filter(|start| *start == 0 || matches!(frame.as_bytes()[start - 1], b'(' | b' '))
        else {
            output.push_str(frame);
            continue;
        };
        let coordinates = &frame[start + "server.js:".len()..];
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
            .ok_or(Error::MissingMapping(generated_line, generated_column))?;
        let source = token
            .get_source()
            .ok_or(Error::MissingSource(generated_line, generated_column))?;
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
        let mut builder = SourceMapBuilder::new(Some("server.js"));
        builder.add(0, 4, 9, 2, Some("src/page.tsx"), None, false);
        let map = builder.into_sourcemap();
        assert_eq!(
            map_stack(
                "Error: failed\n    at render (server.js:1:5)\n    at other (state.js:1:2)\n    at similar (other-server.js:1:5)",
                &map
            )
            .unwrap(),
            "Error: failed\n    at render (src/page.tsx:10:3)\n    at other (state.js:1:2)\n    at similar (other-server.js:1:5)"
        );
        assert!(map_stack("at render (server.js:0:5)", &map).is_err());
        assert!(map_stack("at render (server.js:1:1)", &map).is_err());
    }
}
