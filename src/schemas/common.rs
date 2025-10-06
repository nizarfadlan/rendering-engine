use poem_openapi::Object;

#[derive(Object, Debug)]
pub struct OkResponse {
    pub message: String,
}

#[derive(Object, Debug)]
pub struct BadRequestResponse {
    pub message: String,
}

#[derive(Object, Debug)]
pub struct UnauthorizedResponse {
    pub message: String,
}

impl Default for UnauthorizedResponse {
    fn default() -> Self {
        Self {
            message: "unauthorized".to_string(),
        }
    }
}

#[derive(Object, Debug)]
pub struct ForbiddenResponse {
    pub message: String,
}

#[derive(Object, Debug)]
pub struct NotFoundResponse {
    pub message: String,
}

#[derive(Object, Debug, Clone)]
pub struct ValidateItem {
    loc: Vec<String>,
    msg: String,
}

#[derive(Object, Debug, Clone)]
pub struct UnprocessableEntityResponse {
    pub detail: Vec<ValidateItem>,
}

impl Default for UnprocessableEntityResponse {
    fn default() -> Self {
        Self::new()
    }
}

impl UnprocessableEntityResponse {
    pub fn new() -> Self {
        Self { detail: vec![] }
    }

    pub fn is_has_error(&self) -> bool {
        !self.detail.is_empty()
    }

    pub fn add_error(&mut self, loc: Vec<String>, msg: String) {
        self.detail.push(ValidateItem { loc, msg });
    }
}

#[derive(Object, Debug)]
pub struct InternalServerErrorResponse {
    pub detail: String,
}

impl InternalServerErrorResponse {
    pub fn new(filepath: &str, function: &str, identifier: &str, err: &str) -> Self {
        let msg = format!(
            "error: on {}::{} iden: {} error: {}",
            filepath, function, identifier, err
        );
        tracing::error!("{}", msg);
        Self {
            detail: msg.to_string(),
        }
    }
}
