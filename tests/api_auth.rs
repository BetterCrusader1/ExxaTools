#[test]
fn writes_need_the_token_and_a_local_origin() {
    let port = 8765u16;
    let token = "a".repeat(64);
    let local = Some("http://127.0.0.1:8765");
    let foreign = Some("https://evil.example");

    assert!(exxa::server::authorize("GET", None, None, &token, port).is_ok(), "reads stay open");
    assert!(exxa::server::authorize("POST", None, None, &token, port).is_err(), "write without token");
    assert!(exxa::server::authorize("POST", None, Some("b".repeat(64).as_str()), &token, port).is_err(), "wrong token");
    assert!(exxa::server::authorize("POST", None, Some(&token), &token, port).is_ok(), "right token");
    assert!(exxa::server::authorize("POST", foreign, Some(&token), &token, port).is_err(), "foreign origin with a stolen token");
    assert!(exxa::server::authorize("POST", local, Some(&token), &token, port).is_ok(), "local origin");
    assert!(exxa::server::authorize("DELETE", None, Some(&token), &token, port).is_ok(), "delete with token");
    assert!(exxa::server::authorize("OPTIONS", foreign, None, &token, port).is_ok(), "preflight allowed");
}
