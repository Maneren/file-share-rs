use cfg_if::cfg_if;
use http::status::StatusCode;
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_axum::ResponseOptions;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AppError {
    #[error("Not Found")]
    NotFound,
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
        }
    }
}

#[component]
pub fn ErrorTemplate(
    #[prop(optional)] outside_errors: Option<Errors>,
    #[prop(optional)] errors: Option<RwSignal<Errors>>,
) -> impl IntoView {
    let errors_signal = outside_errors
        .map(RwSignal::new)
        .or(errors)
        .unwrap_or_else(RwSignal::default);

    let errors = Memo::new(move |_| {
        errors_signal()
            .into_iter()
            .filter_map(|(_k, v)| v.downcast_ref::<AppError>().cloned())
            .collect::<Vec<AppError>>()
    });

    cfg_if! { if #[cfg(feature="ssr")] {
        let response = use_context::<ResponseOptions>();
        if let (Some(response), Some(first)) = (response, errors().first()) {
            response.set_status(first.status_code());
        }
    }}

    view! {
      <h1>{move || if errors().len() > 1 { "Errors" } else { "Error" }}</h1>
      <For
        each=move || errors().into_iter().enumerate()
        key=|(index, _)| *index
        let:error
      >
        <h2>{error.1.status_code().to_string()}</h2>
        <p>"Error: " {error.1.to_string()}</p>
      </For>
    }
}
