# Libraries

The `libraries/` directory contains Rust crates shared by BCAIP components.

A library exists to prevent a concept, type, or mechanism from being reimplemented by every component that needs it.

## Shared domain libraries

A concept belongs in a shared library when it exists independently of any single component.

A simple test is:

```text
If component X disappears or is replaced,
does this concept still exist?
```

If the answer is yes, the concept must not be owned by X.

For example, an `Executable` remains an `Executable` regardless of which service currently starts, updates, monitors, or manages it.

Types and operations intrinsic to executables therefore belong to the shared executable domain rather than to a particular service.

This allows another implementation to replace the current service without having to recreate the executable model and its generic logic.

## Exchange format ownership

A crate named:

```text
<component>_exchange_format
```

contains the serialized types that are specifically owned by `<component>` and that another component must know in order to communicate with it.

The owning component is the native authority over those serialized forms.

The important question is not:

```text
Does <component> currently perform this operation?
```

The important question is:

```text
Is this serialized type inherently specific to <component>,
or would the same concept still exist without <component>?
```

If the concept would still exist without `<component>`, it belongs in an appropriate shared library instead.

## Communication

Communication concerns the serialized interface used to talk to a component.

For example, a component may expose a WebSocket command that is specific to itself.

A caller must know the serialized representation of that command in order to send it correctly.

Likewise, if the component returns a serialized type that only it owns, the caller must know that type in order to deserialize the binary data.

Those component-specific serialized types belong in:

```text
<component>_exchange_format
```

However, a component-specific exchange format must not duplicate shared domain types.

If a response contains an `Executable`, a version, an executable update, or any other already-shared concept, the exchange format reuses the shared type instead of redefining it as a component-owned type.

## Ownership test

For every type considered for an exchange-format crate, ask:

```text
1. Is this type needed specifically to encode or decode communication with this component?

2. Is this component naturally the only owner of this serialized form?

3. If this component disappeared and another implementation replaced it,
   would this type still need to exist unchanged?
```

If the answer to the third question is yes, the type probably belongs in a shared library rather than in the component's exchange-format crate.

## Replacement rule

A useful architectural check is:

```text
remove <component>
remove <component>_exchange_format
```

After doing that, only concepts genuinely specific to `<component>` should disappear.

Replacing `supervisor` with another implementation may require implementing a new communication interface for that replacement.

It must not require recreating the generic `Executable` model, executable updates, executable lifecycle rules, or other concepts that exist independently of `supervisor`.

## Summary

```text
shared concept
→ shared library
→ survives replacement of individual components

component-specific serialized interface
→ <component>_exchange_format
→ disappears with that component

caller
→ uses the component's exchange format only for the component-specific
  serialization/deserialization contract
→ reuses shared libraries for shared domain data
```

An exchange-format crate therefore represents only the serialized communication surface genuinely owned by its named component. It must not become a second home for domain concepts merely because that component currently operates on them.
