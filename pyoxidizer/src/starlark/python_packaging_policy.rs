// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use {
    super::{
        python_resource::ResourceCollectionContext,
        util::{required_str_arg, required_type_arg},
    },
    linked_hash_map::LinkedHashMap,
    python_packaging::policy::{
        ExtensionModuleFilter, PythonPackagingPolicy, PythonResourcesPolicy,
    },
    starlark::{
        environment::TypeValues,
        eval::call_stack::CallStack,
        starlark_fun, starlark_module, starlark_parse_param_type, starlark_signature,
        starlark_signature_extraction, starlark_signatures,
        values::{
            error::{RuntimeError, UnsupportedOperation, ValueError},
            none::NoneType,
            Mutable, TypedValue, Value, ValueResult,
        },
    },
    std::convert::TryFrom,
    std::ops::Deref,
};

#[derive(Debug, Clone)]
pub struct PythonPackagingPolicyValue {
    pub inner: PythonPackagingPolicy,

    /// Starlark functions to influence PythonResourceAddCollectionContext creation.
    derive_context_callbacks: Vec<Value>,
}

impl PythonPackagingPolicyValue {
    pub fn new(inner: PythonPackagingPolicy) -> Self {
        Self {
            inner,
            derive_context_callbacks: vec![],
        }
    }

    /// Apply this policy to a resource.
    ///
    /// This has the effect of replacing the `PythonResourceAddCollectionContext`
    /// instance with a fresh one derived from the policy. If no context is
    /// currently defined on the resource, a new one will be created so there is.
    pub fn apply_to_resource<T>(
        &self,
        type_values: &TypeValues,
        call_stack: &mut CallStack,
        value: &mut T,
    ) -> ValueResult
    where
        T: TypedValue + ResourceCollectionContext + Clone,
    {
        let new_context = self
            .inner
            .derive_add_collection_context(&value.as_python_resource());
        value.add_collection_context_mut().replace(new_context);

        for func in &self.derive_context_callbacks {
            // This is a bit wonky. We pass in a `TypeValue`, which isn't a `Value`.
            // To go from `TypeValue` to `Value`, we need to construct a `Value`, which
            // takes ownership of the `TypeValue`. But we need to move a `Value` as an
            // argument into call().
            //
            // Our solution for this is to create a copy of the passed object and
            // construct a `Value` from it. After the call, we downcast it back to
            // our T, retrieve its add context, and replace that on the original value.
            //
            // There might be a way to pass a `Value` into this method. But for now,
            // this solution works.
            let temp_value = Value::new(value.clone());

            func.call(
                call_stack,
                type_values,
                vec![Value::new(self.clone()), temp_value.clone()],
                LinkedHashMap::new(),
                None,
                None,
            )?;

            let downcast_value = temp_value.downcast_ref::<T>().unwrap();
            let inner: &T = downcast_value.deref();
            value
                .add_collection_context_mut()
                .replace(inner.add_collection_context().as_ref().unwrap().clone());
        }

        Ok(Value::from(NoneType::None))
    }
}

impl TypedValue for PythonPackagingPolicyValue {
    type Holder = Mutable<PythonPackagingPolicyValue>;
    const TYPE: &'static str = "PythonPackagingPolicy";

    fn values_for_descendant_check_and_freeze<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = Value> + 'a> {
        Box::new(self.derive_context_callbacks.iter().cloned())
    }

    fn get_attr(&self, attribute: &str) -> ValueResult {
        let v = match attribute {
            "bytecode_optimize_level_zero" => {
                Value::from(self.inner.bytecode_optimize_level_zero())
            }
            "bytecode_optimize_level_one" => Value::from(self.inner.bytecode_optimize_level_one()),
            "bytecode_optimize_level_two" => Value::from(self.inner.bytecode_optimize_level_two()),
            "extension_module_filter" => Value::from(self.inner.extension_module_filter().as_ref()),
            "include_distribution_sources" => {
                Value::from(self.inner.include_distribution_sources())
            }
            "include_distribution_resources" => {
                Value::from(self.inner.include_distribution_resources())
            }
            "include_non_distribution_sources" => {
                Value::from(self.inner.include_non_distribution_sources())
            }
            "include_test" => Value::from(self.inner.include_test()),
            "preferred_extension_module_variants" => {
                Value::try_from(self.inner.preferred_extension_module_variants().clone())?
            }
            "resources_policy" => Value::new::<String>(self.inner.resources_policy().into()),
            attr => {
                return Err(ValueError::OperationNotSupported {
                    op: UnsupportedOperation::GetAttr(attr.to_string()),
                    left: "PythonPackagingPolicy".to_string(),
                    right: None,
                })
            }
        };

        Ok(v)
    }

    fn has_attr(&self, attribute: &str) -> Result<bool, ValueError> {
        Ok(match attribute {
            "bytecode_optimize_level_zero" => true,
            "bytecode_optimize_level_one" => true,
            "bytecode_optimize_level_two" => true,
            "extension_module_filter" => true,
            "include_distribution_sources" => true,
            "include_distribution_resources" => true,
            "include_non_distribution_sources" => true,
            "include_test" => true,
            "preferred_extension_module_variants" => true,
            "resources_policy" => true,
            _ => false,
        })
    }

    fn set_attr(&mut self, attribute: &str, value: Value) -> Result<(), ValueError> {
        match attribute {
            "bytecode_optimize_level_zero" => {
                self.inner.set_bytecode_optimize_level_zero(value.to_bool());
            }
            "bytecode_optimize_level_one" => {
                self.inner.set_bytecode_optimize_level_one(value.to_bool());
            }
            "bytecode_optimize_level_two" => {
                self.inner.set_bytecode_optimize_level_two(value.to_bool());
            }
            "extension_module_filter" => {
                let filter =
                    ExtensionModuleFilter::try_from(value.to_string().as_str()).map_err(|e| {
                        ValueError::from(RuntimeError {
                            code: "PYOXIDIZER_BUILD",
                            message: e,
                            label: format!("{}.{} = {}", Self::TYPE, attribute, value.to_string()),
                        })
                    })?;

                self.inner.set_extension_module_filter(filter);
            }
            "include_distribution_sources" => {
                self.inner.set_include_distribution_sources(value.to_bool());
            }
            "include_distribution_resources" => {
                self.inner
                    .set_include_distribution_resources(value.to_bool());
            }
            "include_non_distribution_sources" => {
                self.inner
                    .set_include_non_distribution_sources(value.to_bool());
            }
            "include_test" => {
                self.inner.set_include_test(value.to_bool());
            }
            "resources_policy" => {
                let policy =
                    PythonResourcesPolicy::try_from(value.to_string().as_str()).map_err(|e| {
                        ValueError::from(RuntimeError {
                            code: "PYOXIDIZER_BUILD",
                            message: e.to_string(),
                            label: format!("{}.{} = {}", Self::TYPE, attribute, value.to_string()),
                        })
                    })?;

                self.inner.set_resources_policy(policy);
            }
            attr => {
                return Err(ValueError::OperationNotSupported {
                    op: UnsupportedOperation::SetAttr(attr.to_string()),
                    left: Self::TYPE.to_owned(),
                    right: None,
                });
            }
        }

        Ok(())
    }
}

// Starlark methods.
impl PythonPackagingPolicyValue {
    fn starlark_register_resource_callback(&mut self, func: &Value) -> ValueResult {
        required_type_arg("func", "function", func)?;

        self.derive_context_callbacks.push(func.clone());

        Ok(Value::from(NoneType::None))
    }

    fn starlark_set_preferred_extension_module_variant(
        &mut self,
        name: &Value,
        value: &Value,
    ) -> ValueResult {
        let name = required_str_arg("name", name)?;
        let value = required_str_arg("value", value)?;

        self.inner
            .set_preferred_extension_module_variant(&name, &value);

        Ok(Value::from(NoneType::None))
    }
}

starlark_module! { python_packaging_policy_module =>
    PythonPackagingPolicy.register_resource_callback(this, func) {
        match this.clone().downcast_mut::<PythonPackagingPolicyValue>()? {
            Some(mut policy) => policy.starlark_register_resource_callback(&func),
            None => Err(ValueError::IncorrectParameterType),
        }
    }

    PythonPackagingPolicy.set_preferred_extension_module_variant(this, name, value) {
        match this.clone().downcast_mut::<PythonPackagingPolicyValue>()? {
            Some(mut policy) => policy.starlark_set_preferred_extension_module_variant(&name, &value),
            None => Err(ValueError::IncorrectParameterType),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::super::python_distribution::PythonDistribution, super::super::testutil::*, super::*,
    };

    #[test]
    fn test_basic() {
        let (mut env, type_values) = starlark_env();

        starlark_eval_in_env(
            &mut env,
            &type_values,
            "dist = default_python_distribution()",
        )
        .unwrap();
        starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy = dist.make_python_packaging_policy()",
        )
        .unwrap();

        let dist_value = starlark_eval_in_env(&mut env, &type_values, "dist").unwrap();
        let dist = dist_value.downcast_ref::<PythonDistribution>().unwrap();

        let policy = dist
            .distribution
            .as_ref()
            .unwrap()
            .create_packaging_policy()
            .unwrap();

        // Need value to go out of scope to avoid double borrow.
        {
            let policy_value = starlark_eval_in_env(&mut env, &type_values, "policy").unwrap();
            assert_eq!(policy_value.get_type(), "PythonPackagingPolicy");

            let x = policy_value
                .downcast_ref::<PythonPackagingPolicyValue>()
                .unwrap();

            // Distribution method should return a policy equivalent to what Starlark gives us.
            assert_eq!(policy, x.inner);
        }

        // attributes work
        let value =
            starlark_eval_in_env(&mut env, &type_values, "policy.extension_module_filter").unwrap();
        assert_eq!(value.get_type(), "string");
        assert_eq!(value.to_string(), policy.extension_module_filter().as_ref());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.extension_module_filter = 'minimal'; policy.extension_module_filter",
        )
        .unwrap();
        assert_eq!(value.to_string(), "minimal");

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_distribution_sources",
        )
        .unwrap();
        assert_eq!(value.get_type(), "bool");
        assert_eq!(value.to_bool(), policy.include_distribution_sources());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_distribution_sources = False; policy.include_distribution_sources",
        )
        .unwrap();
        assert!(!value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_distribution_sources = True; policy.include_distribution_sources",
        )
        .unwrap();
        assert!(value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_distribution_resources",
        )
        .unwrap();
        assert_eq!(value.get_type(), "bool");
        assert_eq!(value.to_bool(), policy.include_distribution_resources());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_distribution_resources = False; policy.include_distribution_resources",
        )
        .unwrap();
        assert!(!value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_distribution_resources = True; policy.include_distribution_resources",
        )
        .unwrap();
        assert!(value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_non_distribution_sources",
        )
        .unwrap();
        assert_eq!(value.get_type(), "bool");
        assert_eq!(value.to_bool(), policy.include_non_distribution_sources());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_non_distribution_sources = False; policy.include_non_distribution_sources",
        )
        .unwrap();
        assert!(!value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_non_distribution_sources = True; policy.include_non_distribution_sources",
        )
        .unwrap();
        assert!(value.to_bool());

        let value = starlark_eval_in_env(&mut env, &type_values, "policy.include_test").unwrap();
        assert_eq!(value.get_type(), "bool");
        assert_eq!(value.to_bool(), policy.include_test());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_test = False; policy.include_test",
        )
        .unwrap();
        assert!(!value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.include_test = True; policy.include_test",
        )
        .unwrap();
        assert!(value.to_bool());

        let value =
            starlark_eval_in_env(&mut env, &type_values, "policy.resources_policy").unwrap();
        assert_eq!(value.get_type(), "string");
        assert_eq!(
            &PythonResourcesPolicy::try_from(value.to_string().as_str()).unwrap(),
            policy.resources_policy()
        );

        // bytecode_optimize_level_zero
        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.bytecode_optimize_level_zero",
        )
        .unwrap();
        assert_eq!(value.get_type(), "bool");
        assert_eq!(value.to_bool(), policy.bytecode_optimize_level_zero());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.bytecode_optimize_level_zero = False; policy.bytecode_optimize_level_zero",
        )
        .unwrap();
        assert!(!value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.bytecode_optimize_level_zero = True; policy.bytecode_optimize_level_zero",
        )
        .unwrap();
        assert!(value.to_bool());

        // bytecode_optimize_level_one
        let value =
            starlark_eval_in_env(&mut env, &type_values, "policy.bytecode_optimize_level_one")
                .unwrap();
        assert_eq!(value.get_type(), "bool");
        assert_eq!(value.to_bool(), policy.bytecode_optimize_level_one());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.bytecode_optimize_level_one = False; policy.bytecode_optimize_level_one",
        )
        .unwrap();
        assert!(!value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.bytecode_optimize_level_one = True; policy.bytecode_optimize_level_one",
        )
        .unwrap();
        assert!(value.to_bool());

        // bytecode_optimize_level_two
        let value =
            starlark_eval_in_env(&mut env, &type_values, "policy.bytecode_optimize_level_two")
                .unwrap();
        assert_eq!(value.get_type(), "bool");
        assert_eq!(value.to_bool(), policy.bytecode_optimize_level_two());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.bytecode_optimize_level_two = False; policy.bytecode_optimize_level_two",
        )
        .unwrap();
        assert!(!value.to_bool());

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.bytecode_optimize_level_two = True; policy.bytecode_optimize_level_two",
        )
        .unwrap();
        assert!(value.to_bool());
    }

    #[test]
    fn test_preferred_extension_module_variants() {
        let (mut env, type_values) = starlark_env();

        starlark_eval_in_env(
            &mut env,
            &type_values,
            "dist = default_python_distribution()",
        )
        .unwrap();
        starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy = dist.make_python_packaging_policy()",
        )
        .unwrap();

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.preferred_extension_module_variants",
        )
        .unwrap();
        assert_eq!(value.get_type(), "dict");
        assert_eq!(value.length().unwrap(), 0);

        starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.set_preferred_extension_module_variant('foo', 'bar')",
        )
        .unwrap();

        let value = starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.preferred_extension_module_variants",
        )
        .unwrap();
        assert_eq!(value.get_type(), "dict");
        assert_eq!(value.length().unwrap(), 1);
        assert_eq!(value.at(Value::from("foo")).unwrap(), Value::from("bar"));
    }

    #[test]
    fn test_register_resource_callback() {
        let (mut env, type_values) = starlark_env();

        starlark_eval_in_env(
            &mut env,
            &type_values,
            "dist = default_python_distribution()",
        )
        .unwrap();
        starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy = dist.make_python_packaging_policy()",
        )
        .unwrap();
        starlark_eval_in_env(
            &mut env,
            &type_values,
            "def my_func(policy, resource):\n    return None",
        )
        .unwrap();

        starlark_eval_in_env(
            &mut env,
            &type_values,
            "policy.register_resource_callback(my_func)",
        )
        .unwrap();

        let policy_value = starlark_eval_in_env(&mut env, &type_values, "policy").unwrap();
        let policy = policy_value
            .downcast_ref::<PythonPackagingPolicyValue>()
            .unwrap();
        assert_eq!(policy.derive_context_callbacks.len(), 1);

        let func = policy.derive_context_callbacks[0].clone();
        assert_eq!(func.get_type(), "function");
        assert_eq!(func.to_str(), "my_func(policy, resource)");
    }
}
