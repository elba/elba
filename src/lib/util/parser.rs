use nom::{
    bytes::complete::{tag, take_while1},
    character::complete::{space0, space1},
    IResult,
};

pub fn find_imports(src: &str, is_literal: bool) -> Vec<Module> {
    src.lines()
        .filter_map(|line| {
            let i = if is_literal {
                parse_literal_start(line).ok()?.0
            } else {
                line
            };
            parse_import(i).map(|(_, m)| m).ok()
        })
        .collect()
}

fn parse_literal_start(i: &str) -> IResult<&str, ()> {
    let (i, _) = space0(i)?;
    let (i, _) = tag(">")(i)?;
    let (i, _) = space1(i)?;
    Ok((i, ()))
}

fn parse_import(i: &str) -> IResult<&str, Module> {
    let (i, _) = space0(i)?;
    let (i, _) = tag("import")(i)?;
    let (i, _) = space1(i)?;
    let (i, module) = take_while1(|c: char| c == '.' || c.is_ascii_alphanumeric())(i)?;
    Ok((i, Module(module.to_string())))
}

#[derive(Debug, PartialEq, Eq)]
pub struct Module(pub String);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_import() {
        let src = r#"
module Main

import Btree
import Btree.Node

main : IO ()
main = do let t = toTree [1,8,2,7,9,3]
    print (Btree.toList t)
"#;
        assert_eq!(
            find_imports(src, false),
            vec![
                Module("Btree".to_string()),
                Module("Btree.Node".to_string())
            ]
        );
    }

    #[test]
    fn test_parse_import_literal() {
        let src = r#"
import This.Is.Comment

> import Btree # This.Is.Comment.Too
> import Btree.Node
"#;
        assert_eq!(
            find_imports(src, true),
            vec![
                Module("Btree".to_string()),
                Module("Btree.Node".to_string())
            ]
        );
    }
}
