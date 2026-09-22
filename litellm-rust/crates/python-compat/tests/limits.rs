use litellm_python_compat::{Error, MAX_DEPTH, Value, json, literal::literal_eval, pickle};

fn nested_list(depth: usize) -> Value {
    (0..depth).fold(Value::from(1), |value, _| Value::List(vec![value]))
}

/// A protocol 3 pickle of `depth` nested lists around `1`: `EMPTY_LIST` per level, then
/// `BININT1 1`, then `APPEND` per level. Written by hand because `dumps` refuses the depth.
fn nested_list_pickle(depth: usize) -> Vec<u8> {
    let mut data = vec![0x80, 3];
    data.extend(std::iter::repeat_n(b']', depth));
    data.extend([b'K', 1]);
    data.extend(std::iter::repeat_n(b'a', depth));
    data.push(b'.');
    data
}

#[test]
fn literal_eval_rejects_nesting_beyond_the_limit() {
    let at_limit = format!("{}1{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
    assert!(literal_eval(&at_limit).is_ok());

    let beyond = format!(
        "{}1{}",
        "[".repeat(MAX_DEPTH + 1),
        "]".repeat(MAX_DEPTH + 1)
    );
    assert!(matches!(literal_eval(&beyond), Err(Error::TooDeep)));
}

#[test]
fn literal_eval_ignores_brackets_inside_strings() {
    let text = format!("'{}'", "[".repeat(MAX_DEPTH + 1));
    assert!(matches!(literal_eval(&text), Ok(Value::Str(_))));
}

#[test]
fn pickle_nesting_is_bounded_in_both_directions() {
    assert_eq!(
        pickle::loads(&nested_list_pickle(MAX_DEPTH)).unwrap(),
        nested_list(MAX_DEPTH)
    );
    assert!(matches!(
        pickle::loads(&nested_list_pickle(MAX_DEPTH + 1)),
        Err(Error::InvalidPickle(_))
    ));
    assert!(pickle::dumps(&nested_list(MAX_DEPTH)).is_ok());
    assert!(matches!(
        pickle::dumps(&nested_list(MAX_DEPTH + 2)),
        Err(Error::TooDeep)
    ));
}

#[test]
fn pickle_dumps_refuses_types_it_would_change() {
    assert!(matches!(
        pickle::dumps(&literal_eval("{1, 2}").unwrap()),
        Err(Error::NotPicklable("set"))
    ));
    assert!(matches!(
        pickle::dumps(&literal_eval("1+2j").unwrap()),
        Err(Error::NotPicklable("complex"))
    ));
}

#[test]
fn pickle_loads_rejects_class_references_and_trailing_data() {
    assert!(matches!(
        pickle::loads(b"\x80\x02c__builtin__\ncomplex\nq\x00."),
        Err(Error::InvalidPickle(_))
    ));
    let mut data = pickle::dumps(&literal_eval("{'a': 1}").unwrap()).unwrap();
    data.push(b'.');
    assert!(matches!(pickle::loads(&data), Err(Error::InvalidPickle(_))));
}

#[test]
fn to_json_reports_what_python_json_dumps_would_reject() {
    let error = json::to_json(&Value::Bytes(b"x".to_vec())).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Object of type bytes is not JSON serializable"
    );
    assert!(matches!(
        json::to_json(&Value::Float(f64::NAN)),
        Err(Error::NonFiniteFloat)
    ));
    assert_eq!(json::dumps(&Value::Float(f64::NAN)).unwrap(), "NaN");
}
