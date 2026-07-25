use jni::bind_java_type;

jni::bind_java_type! {
    pub(crate) JavaSystem => "java.lang.System",
    methods {
        /// Returns the same hash code for the given object as would be returned by the default method hashCode(),
        /// whether or not the given object's class overrides hashCode(). As much as is reasonably practical,
        /// the default `hashCode` method defined by class `Object` does return distinct integers for distinct objects.
        static fn identity_hash_code(obj: JObject) -> jint,
    }
}

bind_java_type! {
    pub(crate) JProxy => "java.lang.reflect.Proxy",
    type_map = {
        JInvocationHandler => "java.lang.reflect.InvocationHandler",
    },
    methods {
        static fn new_proxy_instance(
            class_loader: JClassLoader,
            interfaces: JClass[],
            invoc_hdl: JInvocationHandler
        ) -> JObject,
    },
}

bind_java_type! {
    pub(crate) JInvocationHandler => "java.lang.reflect.InvocationHandler",
}

bind_java_type! {
    pub JMethod => "java.lang.reflect.Method",
    methods {
        fn equals(arg0: JObject) -> jboolean,
        fn get_name() -> JString,
        fn get_parameter_types() -> JClass[],
        fn get_return_type() -> JClass,
    },
}

bind_java_type! {
    pub JBoolean => "java.lang.Boolean",
    constructors {
        fn new(arg0: jboolean),
    },
    methods {
        fn value {
            name = "booleanValue",
            sig = () -> jboolean,
        },
    },
}

bind_java_type! {
    pub JCharacter => "java.lang.Character",
    constructors {
        fn new(arg0: jchar),
    },
    methods {
        fn value {
            name = "charValue",
            sig = () -> jchar,
        },
    }
}

bind_java_type! {
    pub JNumber => "java.lang.Number",
    constructors {
        fn new(),
    },
    methods {
        fn byte_value() -> jbyte,
        fn double_value() -> jdouble,
        fn float_value() -> jfloat,
        fn int_value() -> jint,
        fn long_value() -> jlong,
        fn short_value() -> jshort,
    },
}

bind_java_type! {
    pub JByte => "java.lang.Byte",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jbyte),
    },
    methods {
        fn value {
            name = "byteValue",
            sig = () -> jbyte,
        },
        static fn parse_byte(arg0: JString) -> jbyte,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JShort => "java.lang.Short",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jshort),
    },
    methods {
        fn value {
            name = "shortValue",
            sig = () -> jshort,
        },
        static fn parse_short(arg0: JString) -> jshort,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JInteger => "java.lang.Integer",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jint),
    },
    methods {
        fn value {
            name = "intValue",
            sig = () -> jint,
        },
        static fn parse_int(arg0: JString) -> jint,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JLong => "java.lang.Long",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jlong),
    },
    methods {
        fn value {
            name = "longValue",
            sig = () -> jlong,
        },
        static fn parse_long(arg0: JString) -> jlong,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JFloat => "java.lang.Float",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jfloat),
    },
    methods {
        fn value {
            name = "floatValue",
            sig = () -> jfloat,
        },
        static fn parse_float(arg0: JString) -> jfloat,
    },
    is_instance_of = {
        number: JNumber,
    },
}

bind_java_type! {
    pub JDouble => "java.lang.Double",
    type_map = {
        JNumber => "java.lang.Number",
    },
    constructors {
        fn new(arg0: jdouble),
    },
    methods {
        fn value {
            name = "doubleValue",
            sig = () -> jdouble,
        },
        static fn parse_double(arg0: JString) -> jdouble,
    },
    is_instance_of = {
        number: JNumber,
    },
}

#[test]
#[cfg(not(target_os = "android"))]
fn verify_bindings() {
    use crate::{jni_init_vm_for_unit_test, jni_with_env};
    jni_init_vm_for_unit_test();
    jni_with_env(|env| {
        let ctx = jni::refs::LoaderContext::None;
        JavaSystemAPI::get(env, &ctx).unwrap();
        JProxyAPI::get(env, &ctx).unwrap();
        JInvocationHandlerAPI::get(env, &ctx).unwrap();
        JMethodAPI::get(env, &ctx).unwrap();
        JBooleanAPI::get(env, &ctx).unwrap();
        JCharacterAPI::get(env, &ctx).unwrap();
        JNumberAPI::get(env, &ctx).unwrap();
        JByteAPI::get(env, &ctx).unwrap();
        JShortAPI::get(env, &ctx).unwrap();
        JIntegerAPI::get(env, &ctx).unwrap();
        JLongAPI::get(env, &ctx).unwrap();
        JFloatAPI::get(env, &ctx).unwrap();
        JDoubleAPI::get(env, &ctx).unwrap();

        let jinteger = JInteger::new(env, 1)?;
        let _jnum: JNumber = JNumber::cast_local(env, jinteger)?;

        Ok::<_, jni::errors::Error>(())
    })
    .unwrap();
}
