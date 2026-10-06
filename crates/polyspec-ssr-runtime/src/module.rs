use crate::Error;
use deno_core::v8;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

mod path;

pub(crate) use path::Sources;
use path::resolve;
pub(crate) use path::valid_path;

struct Modules {
    sources: Arc<Sources>,
    compiled: Vec<(String, v8::Global<v8::Module>)>,
}

impl Modules {
    fn new(sources: Arc<Sources>) -> Self {
        Self {
            sources,
            compiled: Vec::new(),
        }
    }

    fn find<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        path: &str,
    ) -> Option<v8::Local<'s, v8::Module>> {
        self.compiled
            .iter()
            .find(|(name, _)| name == path)
            .map(|(_, module)| v8::Local::new(scope, module))
    }

    fn path<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        module: v8::Local<'s, v8::Module>,
    ) -> Option<&str> {
        self.compiled
            .iter()
            .find(|(_, candidate)| v8::Local::new(scope, candidate) == module)
            .map(|(path, _)| path.as_str())
    }
}

fn compile_all<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    modules: &Rc<RefCell<Modules>>,
) -> Result<(), Error> {
    let sources = Arc::clone(&modules.borrow().sources);
    for (path, source) in &sources.files {
        let code = v8::String::new(scope, source).ok_or(Error::InvalidBundle(
            "server JavaScript exceeds V8 string limit",
        ))?;
        let name = v8::String::new(scope, path).ok_or(Error::InvalidBundle(
            "server JavaScript path exceeds V8 string limit",
        ))?;
        let origin = v8::ScriptOrigin::new(
            scope,
            name.into(),
            0,
            0,
            false,
            0,
            None,
            false,
            false,
            true,
            None,
        );
        let mut source = v8::script_compiler::Source::new(code, Some(&origin));
        let module = v8::script_compiler::compile_module(scope, &mut source)
            .ok_or(Error::InvalidBundle("server module compilation failed"))?;
        modules
            .borrow_mut()
            .compiled
            .push((path.clone(), v8::Global::new(scope, module)));
    }
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn resolve_static<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    attributes: v8::Local<'s, v8::FixedArray>,
    referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
    v8::callback_scope!(unsafe scope, context);
    let Some(modules) = context.get_slot::<RefCell<Modules>>() else {
        return throw(scope, "server module loader is unavailable");
    };
    if attributes.length() != 0 {
        return throw(scope, "module import attributes are unsupported");
    }
    let modules = modules.borrow();
    let Some(path) = modules.path(scope, referrer) else {
        return throw(scope, "server module referrer is unknown");
    };
    let specifier_text = specifier.to_rust_string_lossy(scope);
    let Some(roundtrip) = v8::String::new(scope, &specifier_text) else {
        return throw(scope, "server module import path exceeds V8 string limit");
    };
    if !roundtrip.strict_equals(specifier.into()) {
        return throw(scope, "server module import path is invalid Unicode");
    }
    let Some(path) = resolve(path, &specifier_text) else {
        return throw(scope, "server module import path is invalid");
    };
    modules
        .find(scope, &path)
        .or_else(|| throw(scope, "server module import is missing"))
}

fn throw<'s, T>(scope: &mut v8::PinScope<'s, '_>, message: &str) -> Option<T> {
    let text = v8::String::new(scope, message)?;
    let error = v8::Exception::type_error(scope, text);
    scope.throw_exception(error);
    None
}

pub(crate) fn initialize<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Context>,
    sources: Arc<Sources>,
) -> Result<BTreeMap<String, usize>, Error> {
    if context.get_slot::<RefCell<Modules>>().is_some() {
        return Err(Error::InvalidBundle(
            "server module loader already installed",
        ));
    }
    let modules = Rc::new(RefCell::new(Modules::new(Arc::clone(&sources))));
    if context.set_slot(Rc::clone(&modules)).is_some() {
        return Err(Error::InvalidBundle(
            "server module loader already installed",
        ));
    }
    compile_all(scope, &modules)?;
    let entry = modules
        .borrow()
        .find(scope, &sources.entry)
        .ok_or(Error::InvalidBundle("server module entry is missing"))?;
    for (_, module) in &modules.borrow().compiled {
        let module = v8::Local::new(scope, module);
        if module.get_status() == v8::ModuleStatus::Uninstantiated
            && module.instantiate_module(scope, resolve_static) != Some(true)
        {
            return Err(Error::InvalidBundle("server module instantiation failed"));
        }
    }
    let evaluation = entry
        .evaluate(scope)
        .ok_or(Error::InvalidBundle("server module evaluation failed"))?;
    let promise = v8::Local::<v8::Promise>::try_from(evaluation)
        .map_err(|_| Error::InvalidBundle("server module evaluation did not return a promise"))?;
    scope.perform_microtask_checkpoint();
    match promise.state() {
        v8::PromiseState::Fulfilled => {}
        v8::PromiseState::Rejected => {
            let reason = promise.result(scope);
            promise.mark_as_handled();
            let message = reason
                .to_string(scope)
                .ok_or(Error::InvalidBundle(
                    "server module error cannot be converted to text",
                ))?
                .to_rust_string_lossy(scope);
            let stack = if let Ok(object) = v8::Local::<v8::Object>::try_from(reason) {
                let key = v8::String::new(scope, "stack")
                    .ok_or(Error::InvalidBundle("server module stack name unavailable"))?;
                let value = object
                    .get(scope, key.into())
                    .ok_or(Error::InvalidBundle("server module stack lookup failed"))?;
                if value.is_undefined() {
                    None
                } else {
                    Some(
                        value
                            .to_string(scope)
                            .ok_or(Error::InvalidBundle("server module stack is invalid"))?
                            .to_rust_string_lossy(scope),
                    )
                }
            } else {
                None
            };
            return Err(Error::JavaScript { message, stack });
        }
        v8::PromiseState::Pending => {
            return Err(Error::InvalidBundle(
                "server module evaluation did not complete",
            ));
        }
    }
    let namespace = entry.get_module_namespace();
    let namespace = v8::Local::<v8::Object>::try_from(namespace)
        .map_err(|_| Error::InvalidBundle("server module namespace is invalid"))?;
    let key = v8::String::new(scope, "render")
        .ok_or(Error::InvalidBundle("render export name unavailable"))?;
    let render = namespace
        .get(scope, key.into())
        .ok_or(Error::InvalidBundle("render export lookup failed"))?;
    if !render.is_function() {
        return Err(Error::InvalidBundle("render function export required"));
    }
    context.set_embedder_data(0, render);
    let mut indices = BTreeMap::new();
    for (path, module) in &modules.borrow().compiled {
        let module = v8::Local::new(scope, module);
        let index = scope.add_context_data(context, module);
        indices.insert(path.clone(), index);
    }
    Ok(indices)
}

pub(crate) fn render<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Context>,
) -> Result<v8::Local<'s, v8::Function>, Error> {
    let value = context
        .get_embedder_data(scope, 0)
        .ok_or(Error::InvalidBundle("render export is missing"))?;
    v8::Local::<v8::Function>::try_from(value)
        .map_err(|_| Error::InvalidBundle("render function export required"))
}

pub(crate) fn install_dynamic_import_callback(isolate: &mut v8::OwnedIsolate) {
    isolate.set_host_import_module_dynamically_callback(dynamic_import);
}

pub(crate) fn install_sources<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    context: v8::Local<'s, v8::Context>,
    sources: Arc<Sources>,
    indices: &BTreeMap<String, usize>,
) -> Result<(), Error> {
    if context.get_slot::<RefCell<Modules>>().is_some() {
        return Err(Error::Snapshot("server module loader already installed"));
    }
    if indices.len() != sources.files.len()
        || !indices.keys().eq(sources.files.keys())
        || indices
            .values()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != indices.len()
    {
        return Err(Error::Snapshot("server module snapshot indices differ"));
    }
    let mut modules = Modules::new(sources);
    for (path, index) in indices {
        let module = scope
            .get_context_data_from_snapshot_once::<v8::Module>(*index)
            .map_err(|_| Error::Snapshot("server module was not restored"))?;
        modules
            .compiled
            .push((path.clone(), v8::Global::new(scope, module)));
    }
    if context.set_slot(Rc::new(RefCell::new(modules))).is_some() {
        return Err(Error::Snapshot("server module loader already installed"));
    }
    Ok(())
}

pub(crate) struct ContextModules<'s>(v8::Local<'s, v8::Context>);

impl<'s> ContextModules<'s> {
    pub(crate) fn new(context: v8::Local<'s, v8::Context>) -> Self {
        Self(context)
    }
}

impl Drop for ContextModules<'_> {
    fn drop(&mut self) {
        self.0.clear_all_slots();
    }
}

fn reject<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    resolver: v8::Local<'s, v8::PromiseResolver>,
    message: &str,
) -> Option<v8::Local<'s, v8::Promise>> {
    let text = v8::String::new(scope, message)?;
    let error = v8::Exception::type_error(scope, text);
    resolver.reject(scope, error)?;
    Some(resolver.get_promise(scope))
}

fn reject_value<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    resolver: v8::Local<'s, v8::PromiseResolver>,
    value: v8::Local<'s, v8::Value>,
) -> Option<v8::Local<'s, v8::Promise>> {
    resolver.reject(scope, value)?;
    Some(resolver.get_promise(scope))
}

fn dynamic_import<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    _host_options: v8::Local<'s, v8::Data>,
    resource_name: v8::Local<'s, v8::Value>,
    specifier: v8::Local<'s, v8::String>,
    attributes: v8::Local<'s, v8::FixedArray>,
) -> Option<v8::Local<'s, v8::Promise>> {
    let resolver = v8::PromiseResolver::new(scope)?;
    if attributes.length() != 0 {
        return reject(scope, resolver, "module import attributes are unsupported");
    }
    let Ok(resource_name) = v8::Local::<v8::String>::try_from(resource_name) else {
        return reject(scope, resolver, "server module referrer is invalid");
    };
    let resource_name_text = resource_name.to_rust_string_lossy(scope);
    let Some(resource_roundtrip) = v8::String::new(scope, &resource_name_text) else {
        return reject(
            scope,
            resolver,
            "server module referrer exceeds V8 string limit",
        );
    };
    if !resource_name.strict_equals(resource_roundtrip.into()) {
        return reject(scope, resolver, "server module referrer is invalid Unicode");
    }
    let specifier_text = specifier.to_rust_string_lossy(scope);
    let Some(specifier_roundtrip) = v8::String::new(scope, &specifier_text) else {
        return reject(
            scope,
            resolver,
            "server module import path exceeds V8 string limit",
        );
    };
    if !specifier.strict_equals(specifier_roundtrip.into()) {
        return reject(
            scope,
            resolver,
            "server module import path is invalid Unicode",
        );
    }
    let context = scope.get_current_context();
    let Some(modules) = context.get_slot::<RefCell<Modules>>() else {
        return reject(scope, resolver, "server module loader is unavailable");
    };
    if !modules
        .borrow()
        .sources
        .files
        .contains_key(&resource_name_text)
    {
        return reject(scope, resolver, "server module referrer is unknown");
    }
    let Some(path) = resolve(&resource_name_text, &specifier_text) else {
        return reject(scope, resolver, "server module import path is invalid");
    };
    if !modules.borrow().sources.files.contains_key(&path) {
        return reject(scope, resolver, "server module import is missing");
    }
    let Some(module) = modules.borrow().find(scope, &path) else {
        return reject(scope, resolver, "server module import is missing");
    };
    v8::tc_scope!(let scope, scope);
    if module.get_status() == v8::ModuleStatus::Uninstantiated
        && module.instantiate_module(scope, resolve_static) != Some(true)
    {
        if let Some(exception) = scope.exception() {
            return reject_value(scope, resolver, exception);
        }
        return reject(scope, resolver, "server module instantiation failed");
    }
    if module.get_status() == v8::ModuleStatus::Instantiated {
        let Some(evaluation) = module.evaluate(scope) else {
            if let Some(exception) = scope.exception() {
                return reject_value(scope, resolver, exception);
            }
            return reject(scope, resolver, "server module evaluation failed");
        };
        let Ok(evaluation) = v8::Local::<v8::Promise>::try_from(evaluation) else {
            return reject(
                scope,
                resolver,
                "server module evaluation did not return a promise",
            );
        };
        scope.perform_microtask_checkpoint();
        match evaluation.state() {
            v8::PromiseState::Fulfilled => {}
            v8::PromiseState::Rejected => {
                let reason = evaluation.result(scope);
                evaluation.mark_as_handled();
                return reject_value(scope, resolver, reason);
            }
            v8::PromiseState::Pending => {
                return reject(scope, resolver, "server module evaluation did not complete");
            }
        }
    }
    if module.get_status() == v8::ModuleStatus::Errored {
        return reject_value(scope, resolver, module.get_exception());
    }
    if module.get_status() != v8::ModuleStatus::Evaluated {
        return reject(scope, resolver, "server module evaluation did not complete");
    }
    resolver.resolve(scope, module.get_module_namespace())?;
    Some(resolver.get_promise(scope))
}

#[cfg(test)]
mod tests;
