//! Integration test: recorded collector fixtures must deserialize into the
//! models scoring consumes — no live API involved.

#[test]
fn fixtures_deserialize() {
    let ok: scorecards::models::RepoRecord = serde_json::from_str(include_str!(
        "fixtures/nvim-telescope__telescope.nvim.json"
    ))
    .expect("telescope fixture parses");
    assert_eq!(ok.status, scorecards::models::Status::Ok);
    let data = ok.data.expect("ok record carries signals");
    assert!(data.stars > 0);

    let missing: scorecards::models::RepoRecord = serde_json::from_str(include_str!(
        "fixtures/definitely-not__exists-xyz-123.json"
    ))
    .expect("404 fixture parses");
    assert_eq!(missing.status, scorecards::models::Status::NotFound);
    assert!(missing.data.is_none());
}
