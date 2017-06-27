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
    pub json_value<&[u8], JsonValue>,
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
            char!('['),
            separated_list!(
                char!(','),
                json_value
            ),
            char!(']')
        ),
        |elems| JsonValue::Array(elems)
    )
);

named!(
    json_object<&[u8], JsonValue>,
    map!(
        delimited!(
            char!('{'),
            separated_list!(
                char!(','),
                separated_pair!(
                    json_string,
                    char!(':'),
                    json_value
                )
            ),
            char!('}')
        ),
        |pairs| {
            let mut obj = HashMap::new();
            for (key, value) in pairs {
                if let JsonValue::String(key_string) = key {
                    obj.insert(key_string, value);
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
        parse_test!(json_object, "{}", Object(HashMap::new()));
        parse_test!(json_object, "{\"a\":42}", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m
        }));
        parse_test!(json_object, "{\"a\":42,\"b\":43}", Object({
            let mut m = HashMap::new();
            m.insert(Str::from("a"), Int(42));
            m.insert(Str::from("b"), Int(43));
            m
        }));
    }
}
