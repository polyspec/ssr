use crate::{Error, Pool, PoolOptions, ServerBundle, module, snapshot};

impl Pool {
    pub fn new(bundle: ServerBundle, options: PoolOptions) -> Result<Self, Error> {
        options.validate()?;
        let timeout = options.timeout;
        let bundle = module::Sources::new((bundle.entry_path, bundle.entry_bytes), bundle.chunks)?;
        let snapshot = snapshot::get(&bundle, timeout)?;
        Self::from_snapshot(snapshot, options)
    }

    pub fn new_react(
        framework: (String, Vec<u8>),
        application: ServerBundle,
        options: PoolOptions,
    ) -> Result<Self, Error> {
        options.validate()?;
        let timeout = options.timeout;
        if !module::valid_path(&framework.0) || !module::valid_path(&application.entry_path) {
            return Err(Error::InvalidBundle(
                "React server JavaScript path is invalid",
            ));
        }
        if framework.0 == application.entry_path {
            return Err(Error::InvalidBundle(
                "React framework and application paths must differ",
            ));
        }
        if !application.chunks.is_empty() {
            return Err(Error::InvalidBundle(
                "React server chunks are unsupported by the IIFE build",
            ));
        }
        let framework_source = std::str::from_utf8(&framework.1)
            .map_err(|_| Error::InvalidBundle("React framework UTF-8 required"))?;
        let application_source = std::str::from_utf8(&application.entry_bytes)
            .map_err(|_| Error::InvalidBundle("React application UTF-8 required"))?;
        if framework_source.is_empty() || application_source.is_empty() {
            return Err(Error::InvalidBundle("React bundles must be nonempty"));
        }
        let snapshot = snapshot::get_react(
            &framework.0,
            framework_source,
            &application.entry_path,
            application_source,
            timeout,
        )?;
        Self::from_snapshot(snapshot, options)
    }
}
