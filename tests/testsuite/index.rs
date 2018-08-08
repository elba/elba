use super::util::index;
use elba::package::Name;
use std::str::FromStr;

#[test]
fn index_success() {
    let i = index();

    let vs = i.entries(&Name::from_str("no_conflict/root").unwrap());
    vs.unwrap();
}
