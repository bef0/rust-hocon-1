#[macro_use]
extern crate nom;

use nom::*;
use std::collections::HashMap;
use std::string::String;

#[derive(Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Boolean(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>)
}

named!(
    pub json_value_root<&[u8], JsonValue>,
    delimited!(
        json_whitespace,
        alt!(json_object | json_object_root),
        json_whitespace
    )
);

named!(
    json_value<&[u8], JsonValue>,
    alt_complete!(
        json_null |
        json_boolean |
        json_float |
        json_int |
        json_string |
        json_array |
        json_object
    )
);

fn json_whitespace(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let len = input.len();
    let mut i = 0;
    while i < len {
        let c = input[i];
        if c == b' ' || c == b'\t' || c == b'\n' {
            i += 1;
        } else if c == b'#' {
            i += 1;
            while i < len && input[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && i < len - 1 && input[i+1] == b'/' {
            i += 2;
            while i < len && input[i] != b'\n' {
                i += 1;
            }
        } else {
            break;
        }
    }
    return IResult::Done(&input[i..], &input[..i]);
}

fn inferrable_comma(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let len = input.len();
    let mut i = 0;
    let mut got_newline = false;
    let mut got_comma = false;
    while i < len {
        let c = input[i];
        if c == b' ' || c == b'\t' {
            i += 1;
        } else if c == b'\n' {
            got_newline = true;
            i += 1;
        } else if c == b'#' {
            i += 1;
            while i < len && input[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && i < len - 1 && input[i+1] == b'/' {
            i += 2;
            while i < len && input[i] != b'\n' {
                i += 1;
            }
        } else if c == b',' && !got_comma {
            got_comma = true;
            i += 1;
        } else {
            break;
        }
    }
    if got_comma || got_newline {
        return IResult::Done(&input[i..], &input[..i]);
    } else {
        return IResult::Error(error_position!(ErrorKind::Char, b","));
    }
}

named!(
    json_null<&[u8], JsonValue>,
    value!(JsonValue::Null, tag!("null"))
);

named!(
    json_boolean<&[u8], JsonValue>,
    alt!(
        tag!("true") => { |_| JsonValue::Boolean(true) } |
        tag!("false") => { |_| JsonValue::Boolean(false) }
    )
);

named!(
    json_int<&[u8], JsonValue>,
    map!(
        flat_map!(
            recognize!(
                tuple!(
                    opt!(alt!(tag!("+") | tag!("-"))),
                    digit
                )
            ),
            parse_to!(i64)
        ),
        |i:i64| JsonValue::Int(i)
    )
);

named!(
    json_float<&[u8], JsonValue>,
    map!(
        double,
        |i: f64| { JsonValue::Float(i) }
    )
);

fn escaped_string(input: &[u8]) -> IResult<&[u8], Vec<u8>> {
    let len = input.len();
    let mut i = 0;
    let mut s: Vec<u8> = Vec::new();
    while i < len {
        if i < len - 1 && input[i] == b'\\' && input[i+1] == b'"' {
            s.push(b'"');
            i += 2;
        } else if input[i] == b'"' {
            return IResult::Done(&input[i..], s);
        } else {
            s.push(input[i]);
            i += 1;
        }
    }

    return IResult::Incomplete(Needed::Unknown);
}

named!(
    json_string<&[u8], JsonValue>,
    map!(
        map_res!(
            delimited!(
                char!('"'),
                escaped_string,
                char!('"')
            ),
            String::from_utf8
        ),
        |s| JsonValue::String(s)
    )
);

named!(
    json_array<&[u8], JsonValue>,
    map!(
        delimited!(
            tuple!(char!('['), json_whitespace),
            separated_list_complete!(
                inferrable_comma,
                json_value
            ),
            tuple!(json_whitespace, char!(']'))
        ),
        |elems| JsonValue::Array(elems)
    )
);

named!(
    json_object<&[u8], JsonValue>,
    delimited!(
        tuple!(char!('{'), json_whitespace),
        json_object_root,
        tuple!(json_whitespace, char!('}'))
    )
);

fn merge_json(
    old: JsonValue,
    new: JsonValue
) -> JsonValue {
    match (old, new) {
        (JsonValue::Object(mut obj_prev), JsonValue::Object(mut obj_new)) => {
            for (key, value) in obj_new.drain() {
                let new_value = match obj_prev.remove(&key) {
                    Some(old_value) => merge_json(old_value, value),
                    _ => value
                };
                obj_prev.insert(key, new_value);
            }
            JsonValue::Object(obj_prev)
        },
        (_, new) => {
            new
        }
    }
}

named!(
    json_object_root<&[u8], JsonValue>,
    map!(
        separated_list_complete!(
            inferrable_comma,
            tuple!(
                json_string,
                alt!(
                    preceded!(json_whitespace, json_object) |
                    preceded!(
                        tuple!(json_whitespace, alt!(char!(':') | char!('=')), json_whitespace),
                        json_value
                    )
                )
            )
        ),
        |pairs| {
            let mut obj = HashMap::new();

            for (key, value) in pairs {
                if let JsonValue::String(key_string) = key {

                    let new_value = match obj.remove(&key_string) {
                        Some(old_value) => merge_json(old_value, value),
                        None => value
                    };
                    obj.insert(key_string, new_value);

                }
            }

            JsonValue::Object(obj)
        }
    )
);

#[cfg(test)]
mod tests {
    use super::*;
    use super::JsonValue::*;
    use nom::IResult;
    use std::string::String as Str;

    macro_rules! parse_test(
        ($parser: expr, $input: expr, $output: expr) => (
            assert_eq!($parser($input.as_bytes()), IResult::Done(&b""[..], $output))
        )
    );

    #[test] fn test_json_null() {
        parse_test!(json_value, "null", Null);
    }

    #[test] fn test_json_boolean() {
        parse_test!(json_value, "true", Boolean(true));
        parse_test!(json_value, "false", Boolean(false));
    }

    #[test] fn test_json_int() {
        parse_test!(json_value, "0", Int(0));
        parse_test!(json_value, "1", Int(1));
        parse_test!(json_value, "-2", Int(-2));
        parse_test!(json_value, "42", Int(42));
        parse_test!(json_value, "2834293023", Int(2834293023));
    }

    #[test] fn test_json_float() {
        parse_test!(json_value, "0.0", Float(0.0));
        parse_test!(json_value, "4.2", Float(4.2));
        parse_test!(json_value, "-4.2", Float(-4.2));
        parse_test!(json_value, "-4.2e1", Float(-42.0));
        parse_test!(json_value, "-4.2e-2", Float(-0.042));
    }

    #[test] fn test_json_string() {
        parse_test!(json_value, "\"\"", String(Str::from("")));
        parse_test!(json_value, "\"a\"", String(Str::from("a")));
        parse_test!(json_value, "\"ab\"", String(Str::from("ab")));
        parse_test!(json_value, "\"a b\"", String(Str::from("a b")));
        parse_test!(json_value, "\"a\\\"b\"", String(Str::from("a\"b")));
    }

    #[test] fn test_json_array() {
        parse_test!(json_value, "[]", Array(vec![]));
        parse_test!(json_value, "[null]", Array(vec![Null]));
        parse_test!(json_value, "[1,2]", Array(vec![Int(1), Int(2)]));
        parse_test!(json_value, "[1,[2,3]]", Array(vec![Int(1), Array(vec![Int(2), Int(3)])]));
    }

    #[test] fn test_json_object() {
        parse_test!(json_value, "{}", Object(HashMap::new()));
        parse_test!(json_value, "{\"a\":42}", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m
        }));
        parse_test!(json_value, "{\"a\":42,\"b\":43}", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m.insert(Str::from("b"), Int(43));
            m
        }));
    }

    macro_rules! parse_test_eq(
        ($parser: expr, $input: expr) => (
            assert_eq!($parser($input.as_bytes()), IResult::Done(&b""[..], $input.as_bytes()))
        )
    );

    #[test] fn test_comments() {
        parse_test_eq!(json_whitespace, "");
        parse_test_eq!(json_whitespace, "\n");
        parse_test_eq!(json_whitespace, "\n#");
        parse_test_eq!(json_whitespace, "#\n");
        parse_test_eq!(json_whitespace, " ");
        parse_test_eq!(json_whitespace, " #");
        parse_test_eq!(json_whitespace, " # c");
        parse_test_eq!(json_whitespace, " # c\n");
        parse_test_eq!(json_whitespace, " # c\n ");
        parse_test_eq!(json_whitespace, " # c\n  ");
        parse_test_eq!(json_whitespace, " # c\n  ");
        parse_test_eq!(json_whitespace, " # c\n  //");
        parse_test_eq!(json_whitespace, " # c\n  //\n");
        parse_test_eq!(json_whitespace, " # c\n  //\n////");
        parse_test!(json_value, "[ ]", Array(vec![]));
        parse_test!(json_value, "[ 1]", Array(vec![Int(1)]));
        parse_test!(json_value, "[1 ]", Array(vec![Int(1)]));
        parse_test!(json_value, "[ 1 ]", Array(vec![Int(1)]));
        parse_test!(json_value, "[ 1,2 ]", Array(vec![Int(1), Int(2)]));
        parse_test!(json_value, "[ 1 ,2 ]", Array(vec![Int(1), Int(2)]));
        parse_test!(json_value, "[ 1, 2 ]", Array(vec![Int(1), Int(2)]));
        parse_test!(json_value, "[ 1 , 2 ]", Array(vec![Int(1), Int(2)]));
        parse_test!(json_value, "[ 1 , 2,3 ]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[ 1 , 2,3]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[ 1 , 2, 3]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[ 1 , 2 , 3]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[1 , 2 , 3 ]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[ 1 , 2 , 3 ]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[ 1 , #s\n 2 , 3 ]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[ 1 , #s\n\n 2 , 3 ]", Array(vec![Int(1), Int(2), Int(3)]));

        let m0 = || Object(HashMap::new());
        parse_test!(json_value_root, "{}", m0());
        parse_test!(json_value_root, " {} ", m0());
        parse_test!(json_value_root, " { } ", m0());
        parse_test!(json_value_root, " { \n} ", m0());
        parse_test!(json_value_root, " {\n } ", m0());
        parse_test!(json_value_root, " { \n } ", m0());

        let m1 = || {
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(1));
            Object(m)
        };
        parse_test!(json_value_root, "{\"a\":1}", m1());
        parse_test!(json_value_root, " {\"a\":1} ", m1());
        parse_test!(json_value_root, " { \"a\":1} ", m1());
        parse_test!(json_value_root, " {\"a\" :1} ", m1());
        parse_test!(json_value_root, " {\"a\": 1} ", m1());
        parse_test!(json_value_root, " {\"a\":1 } ", m1());
        parse_test!(json_value_root, " { \"a\" : 1 } ", m1());
        parse_test!(json_value_root, "\n{\n\"a\"\n:\n1\n}\n", m1());
        parse_test!(json_value_root, "\n\n{\n\n\"a\"\n\n:\n\n1\n\n}\n\n", m1());
        parse_test!(json_value_root, "\n{\n\"a\"\n:# cmt \n1\n}\n", m1());

        let m2 = || {
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(1));
            m.insert(Str::from("b"), Int(2));
            Object(m)
        };
        parse_test!(json_value_root, "{\"a\":1,\"b\":2}", m2());
        parse_test!(json_value_root, "{\"a\":1 ,\"b\":2}", m2());
        parse_test!(json_value_root, "{\"a\":1, \"b\":2}", m2());
        parse_test!(json_value_root, "{\"a\":1 , \"b\":2}", m2());
        parse_test!(json_value_root, "{\"a\":1 ,\n \"b\":2}", m2());
        parse_test!(json_value_root, "{\"a\":1 ,\n\n \"b\":2}", m2());
    }

    #[test] fn test_comma_inference() {
        parse_test!(json_value, "[1\n2]", Array(vec![Int(1), Int(2)]));
        parse_test!(json_value, "[1#a\n2]", Array(vec![Int(1), Int(2)]));
        parse_test!(json_value, "[1 , 2 \n, 3]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[1 , 2 \n\n\n, 3]", Array(vec![Int(1), Int(2), Int(3)]));
        parse_test!(json_value, "[1 , 2 \n# s\n\n, 3]", Array(vec![Int(1), Int(2), Int(3)]));

        let m2 = || {
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(1));
            m.insert(Str::from("b"), Int(2));
            Object(m)
        };
        parse_test!(json_value_root, "{ \"a\":1\n\"b\":2 }", m2());
        parse_test!(json_value_root, "{ \"a\":1,\n\"b\":2 }", m2());
    }

    #[test] fn test_equals_instead_of_colon() {
        parse_test!(json_value, "{\"a\" = 42}", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m
        }));
        parse_test!(json_value, "{\"a\" = 42,\"b\":43}", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m.insert(Str::from("b"), Int(43));
            m
        }));
    }

    #[test] fn test_skipping_colon_before_object_values() {
        parse_test!(json_value, "{\"a\" = { \"b\":43 }}", Object({
            let mut m1 = HashMap::new();
            m1.insert(Str::from("b"), Int(43));
            let mut m2 = HashMap::new();
            m2.insert(Str::from("a"), Object(m1));
            m2
        }));
        parse_test!(json_value, "{\"a\" { \"b\":43 }}", Object({
            let mut m1 = HashMap::new();
            m1.insert(Str::from("b"), Int(43));
            let mut m2 = HashMap::new();
            m2.insert(Str::from("a"), Object(m1));
            m2
        }));
    }

    #[test] fn test_dropping_braces_on_root_object() {
        parse_test!(json_value_root, "\"a\" = 42", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m
        }));
        parse_test!(json_value_root, "\"a\" = 42\n", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m
        }));
        parse_test!(json_value_root, "\"a\" = 42,\"b\":43", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m.insert(Str::from("b"), Int(43));
            m
        }));
        parse_test!(json_value_root, "\"a\" = 42\n\"b\":43", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m.insert(Str::from("b"), Int(43));
            m
        }));
    }

    #[test] fn test_object_merging() {
        parse_test!(
            json_value_root,
            r#"
"a" { "b": 1 }
"a" { "c": 2 }
"#,
            Object({
                let mut m1 = HashMap::new();
                m1.insert(Str::from("b"), Int(1));
                m1.insert(Str::from("c"), Int(2));
                let mut m2 = HashMap::new();
                m2.insert(Str::from("a"), Object(m1));
                m2
            })
        );

        parse_test!(
            json_value_root,
            r#"
"a" { "b": { "c": 1 } }
"a" { "b": { "d": 2 } }
"#,
            Object({
                let mut m1 = HashMap::new();
                m1.insert(Str::from("c"), Int(1));
                m1.insert(Str::from("d"), Int(2));
                let mut m2 = HashMap::new();
                m2.insert(Str::from("b"), Object(m1));
                let mut m3 = HashMap::new();
                m3.insert(Str::from("a"), Object(m2));
                m3
            })
        );

        parse_test!(
            json_value_root,
            r#"
"a" { "b": { "c": 1 }, "e": 3 }
"a" { "b": { "d": 2 } }
"#,
            Object({
                let mut m1 = HashMap::new();
                m1.insert(Str::from("c"), Int(1));
                m1.insert(Str::from("d"), Int(2));
                let mut m2 = HashMap::new();
                m2.insert(Str::from("b"), Object(m1));
                m2.insert(Str::from("e"), Int(3));
                let mut m3 = HashMap::new();
                m3.insert(Str::from("a"), Object(m2));
                m3
            })
        );
    }

}
