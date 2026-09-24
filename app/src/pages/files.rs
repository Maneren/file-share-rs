use std::{path::PathBuf, sync::Arc};

use leptos::prelude::*;
use leptos_router::{hooks::use_params, params::Params};
use urlencoding::decode;

use crate::{
    AppConfig,
    api::NewFolder,
    components::{ActionBar, Breadcrumbs, ListingBrowser},
};

#[derive(PartialEq, Eq, Params, Debug)]
struct PathQuery {
    path: String,
}

#[component]
#[allow(clippy::must_use_candidate)]
pub fn FilesPage() -> impl IntoView {
    let path_query = use_params::<PathQuery>();

    let path = Memo::new(move |_| {
        path_query
            .read()
            .as_ref()
            .ok()
            .and_then(|query| decode(&query.path).ok())
            .map_or_default(|path| PathBuf::from(path.as_ref()))
    });

    let create_folder_action = ServerAction::<NewFolder>::new();

    let path_signal = Signal::from(path);

    let app_config = expect_context::<Arc<AppConfig>>();

    view! {
        <div class="p-3 App">
            <ActionBar
                path=path_signal
                allow_upload=app_config.allow_upload
                create_folder_action=create_folder_action
            />
            <Breadcrumbs path=path_signal />
            // Snapshot path: the island remounts fresh on every navigation.
            <ListingBrowser path=path.get_untracked() allow_upload=app_config.allow_upload />
        </div>
    }
}
