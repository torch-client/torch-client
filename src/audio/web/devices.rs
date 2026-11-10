pub fn set_device(_name: Option<&'static str>) {}

pub fn fell_back() -> bool {
    false
}

pub fn scan() {}

pub fn next(_current: Option<&'static str>) -> Option<&'static str> {
    None
}

pub fn label(name: &'static str) -> &'static str {
    name
}
