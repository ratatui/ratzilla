use ratatui::layout::Size;

use crate::{
    backend::utils::{get_raw_screen_size, get_raw_window_size},
    error::Error,
};

use web_sys::{
    js_sys::{Array, Function, Reflect},
    wasm_bindgen::{prelude::*, JsValue},
};

/// Sets the document title.
pub fn set_document_title(title: &str) -> Result<(), Error> {
    web_sys::window()
        .ok_or(Error::UnableToRetrieveWindow)?
        .document()
        .ok_or(Error::UnableToRetrieveDocument)?
        .set_title(title);
    Ok(())
}

/// Open a URL in a new tab or the current tab.
pub fn open_url(url: &str, new_tab: bool) -> Result<(), Error> {
    let window = web_sys::window().ok_or(Error::UnableToRetrieveWindow)?;
    if new_tab {
        window.open_with_url(url)?;
    } else {
        window.location().set_href(url)?;
    }
    Ok(())
}

/// Returns `true` if the screen is a mobile device.
pub fn is_mobile() -> bool {
    let user_agent = web_sys::window().and_then(|w| w.navigator().user_agent().ok());
    user_agent.is_some_and(|agent| {
        let agent = agent.to_lowercase();
        agent.contains("mobile") || agent.contains("tablet")
    })
}

/// Returns the number of characters that can fit in the window (viewport of the browser or terminal).
pub fn get_window_size() -> Size {
    let (w, h) = get_raw_window_size();
    // TODO: These are mildly magical numbers... make them more precise
    (w / 10, h / 20).into()
}

/// Returns the number of characters that can fit in the screen (entire physical display).
pub fn get_screen_size() -> Size {
    let (w, h) = get_raw_screen_size();
    // TODO: These are mildly magical numbers... make them more precise
    (w as u16 / 10, h as u16 / 19).into()
}

/// Calls a global JavaScript function by name, with a custom `this` context and an arbitrary number of arguments.
///
/// This function looks up the property `window[name]` on the global window, checks that it is a JavaScript
/// function, and then calls it using `Function.prototype.apply`.
///
/// # Parameters
///
/// * `name` - The name of the JavaScript function (as a property on the global `window` object).
/// * `this` - The value to use as the `this` binding during the function invocation.
/// * `args` - An iterable list of arguments to pass to the function.
///
/// # Type Parameters
///
/// * `S: AsRef<str>` – A type that can be converted to a string slice; used for the name of the function.
/// * `T: Into<JsValue>` – A type that can be converted into a `JsValue`, representing the `this` binding.
/// * `I: IntoIterator` – An iterable collection of function arguments.
/// * `I::Item: Into<JsValue>` – Each argument can be converted into a `JsValue`.
///
/// # Errors
///
/// * `Error::UnableToRetrieveWindow` if the global `window` object cannot be retrieved.
/// * `Error::JsValue(JsValue)` if any of the following JavaScript operations throw:
///   - retrieving the property via `Reflect::get`
///   - converting the property to a [`Function`] via `.dyn_into::<Function>()`
///   - invoking the function via `Function::apply`
///
/// # Examples
///
/// Calling a global JS function with a custom context:
///
/// ```no_run
/// # use web_sys::wasm_bindgen::JsValue;
/// # use ratzilla::utils::call_js_function_with_context;
/// # use ratzilla::error::Error;
/// use web_sys::js_sys::Object;
/// # fn example() -> Result<(), Error> {
/// // Suppose `myObj` is a JS object you want to be the `this` value.
/// let my_obj = JsValue::from(Object::new());
/// let result = call_js_function_with_context(
///     "myJsFunction",  // JavaScript global function name
///     my_obj,          // custom `this` context
///     [JsValue::from(42), JsValue::from("hello")] // arguments
/// )?;
/// # Ok(())
/// # }
/// ```
///
/// Calling a function without caring about `this`:
///
/// ```no_run
/// # use ratzilla::utils::call_js_function_with_context;
/// # use ratzilla::error::Error;
/// # use ratzilla::utils::call_js_function;
/// # use web_sys::wasm_bindgen::JsValue;
/// # fn example() -> Result<(), Error> {
/// // This will set `this` to `null` which in non-strict mode defaults to the global object.
/// let result = call_js_function_with_context("alert", JsValue::NULL, ["Hello from Rust"])?;
/// //Note that we also have a `call_js_function` function that defaults `this` to `null`.
/// let result = call_js_function("alert", ["Hello from Rust"])?;
/// // This is equivalent to the above call.
/// # Ok(())
/// # }
/// ```
pub fn call_js_function_with_context<S, I, T>(name: S, this: T, args: I) -> Result<JsValue, Error>
where
    S: AsRef<str>,
    T: Into<JsValue>,
    I: IntoIterator,
    I::Item: Into<JsValue>,
{
    let window = web_sys::window().ok_or(Error::UnableToRetrieveWindow)?;
    let func_val = Reflect::get(&window, &JsValue::from_str(name.as_ref()))?;
    let func = func_val.dyn_into::<Function>()?;
    let param_array: Array = args.into_iter().map(Into::into).collect();
    let result = func.apply(&this.into(), &param_array)?;
    Ok(result)
}

/// Calls a global JavaScript function by name, defaulting the `this` context to `null`.
///
/// This is a convenience wrapper around [`call_js_function_with_context`]. It allows callers to
/// simply provide the function name and an iterable of arguments without worrying about the `this`
/// binding. Passing `null` as `this` means that in non‑strict mode, JavaScript will default to
/// using the global `window` object.
///
/// # Parameters
///
/// * `name` - The name of the JavaScript function (a property on the global `window` object).
/// * `args` - An iterable list of arguments to pass to the function call.
///
/// # Type Parameters
///
/// * `S: AsRef<str>` – Type for representing the name of the function.
/// * `I: IntoIterator` – An iterable collection of function arguments.
/// * `I::Item: Into<JsValue>` – Each argument can be converted into a `JsValue`.
///
/// # Errors
/// See [`call_js_function_with_context`] for error details.
///
/// # Examples
///
/// Calling a global function like `alert`:
///
/// ```no_run
/// # use ratzilla::utils::call_js_function;
/// # use ratzilla::error::Error;
/// # fn example() -> Result<(), Error> {
/// // Calls alert("Hello World!") on the global window.
/// call_js_function("alert", ["Hello World!"])?;
/// # Ok(())
/// # }
/// ```
pub fn call_js_function<S, I>(name: S, args: I) -> Result<JsValue, Error>
where
    S: AsRef<str>,
    I: IntoIterator,
    I::Item: Into<JsValue>,
{
    call_js_function_with_context(name, JsValue::NULL, args)
}
