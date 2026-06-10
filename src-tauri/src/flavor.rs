use serde::Serialize;

#[cfg(all(feature = "flavor-full", feature = "flavor-online"))]
compile_error!("只能启用一个 Desktop flavor feature：flavor-full 或 flavor-online。");

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopFlavor {
    Full,
    Online,
    Unspecified,
}

pub fn active_flavor() -> DesktopFlavor {
    #[cfg(feature = "flavor-full")]
    {
        return DesktopFlavor::Full;
    }

    #[cfg(feature = "flavor-online")]
    {
        return DesktopFlavor::Online;
    }

    #[cfg(not(any(feature = "flavor-full", feature = "flavor-online")))]
    {
        DesktopFlavor::Unspecified
    }
}

impl DesktopFlavor {
    pub fn label(self) -> &'static str {
        match self {
            DesktopFlavor::Full => "Full",
            DesktopFlavor::Online => "Online",
            DesktopFlavor::Unspecified => "未指定",
        }
    }

    pub fn product_name(self) -> &'static str {
        match self {
            DesktopFlavor::Full => "HDX Desktop Full",
            DesktopFlavor::Online => "HDX Desktop Online",
            DesktopFlavor::Unspecified => "HDX Desktop",
        }
    }

    pub fn includes_full_backend(self) -> bool {
        matches!(self, DesktopFlavor::Full)
    }

    pub fn remote_endpoint_required(self) -> bool {
        matches!(self, DesktopFlavor::Online)
    }

    pub fn local_actor(self) -> Option<&'static str> {
        match self {
            DesktopFlavor::Full => Some("LOCAL_ADMIN:local-admin"),
            DesktopFlavor::Online | DesktopFlavor::Unspecified => None,
        }
    }
}
