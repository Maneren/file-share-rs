//! Login page: token form shown when `--auth-token` is set.
//!
//! Plain `POST /login` (works without JS); only the show/hide toggle is an
//! island. The server re-validates `next` on POST.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_query_map;

/// Rough client-side mirror of the server's redirect guard; the server
/// re-validates, this just avoids echoing obvious junk into the form.
fn looks_same_origin(next: &str) -> bool {
    next.starts_with('/') && !next.starts_with("//")
}

#[component]
#[allow(clippy::must_use_candidate)]
pub fn LoginPage() -> impl IntoView {
    let query = use_query_map();

    // Snapshot: a navigation remounts the island below, so plain props do.
    let next = query
        .read()
        .get("next")
        .filter(|next| looks_same_origin(next))
        .unwrap_or_else(|| "/".to_string());
    let failed = query.read().get("error").is_some();

    view! {
        <Title text="Login — File Share" />
        <div class="flex justify-center items-center p-3 min-h-[70vh]">
            <div class="w-full max-w-sm shadow-xl card bg-base-100">
                <div class="card-body">
                    <h1 class="card-title">Login required</h1>
                    <p class="text-sm opacity-70">
                        "This server is protected. Enter the access token to continue."
                    </p>
                    <LoginForm next failed />
                </div>
            </div>
        </div>
    }
}

#[island]
fn LoginForm(next: String, failed: bool) -> impl IntoView {
    let (show, set_show) = signal(false);

    view! {
        {failed
            .then(|| {
                view! {
                    <div class="alert alert-error" role="alert">
                        <span>"Invalid access token."</span>
                    </div>
                }
            })}
        <form method="post" action="/login" class="flex flex-col gap-3">
            <input type="hidden" name="next" value=next />
            <label class="flex flex-col gap-1">
                <span class="text-sm">Access token</span>
                <span class="flex gap-2">
                    <input
                        type=move || if show.get() { "text" } else { "password" }
                        name="token"
                        required
                        autofocus
                        placeholder="Access token"
                        autocomplete="current-password"
                        class="grow input input-bordered"
                    />
                    <button
                        type="button"
                        class="btn btn-square btn-ghost"
                        aria-label=move || if show.get() { "Hide token" } else { "Show token" }
                        title=move || if show.get() { "Hide token" } else { "Show token" }
                        on:click=move |_| set_show.update(|show| *show = !*show)
                    >
                        {move || if show.get() { "Hide" } else { "Show" }}
                    </button>
                </span>
            </label>
            <button type="submit" class="btn btn-primary">
                Log in
            </button>
        </form>
    }
}
