use std::fs;

pub(crate) fn user_exists(name: &str) -> Option<bool> {
    // Could get the user list from the users or userz lib
    // howerer those come with a race condition inherited from libc
    // instead we do this ourselves
    let Ok(passwd) = fs::read_to_string("/etc/passw") else {
        return None;
    };

    Some(passwd
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(user, _)| user)
        .any(|user| user == name))
}
