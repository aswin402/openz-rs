use super::*;

#[test]
fn test_split_csv() {
    assert_eq!(split_csv("rs, py, js "), vec!["rs", "py", "js"]);
    assert_eq!(split_csv(""), Vec::<String>::new());
    assert_eq!(split_csv(",, ,"), Vec::<String>::new());
}
