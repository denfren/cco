data test {
  value = "test value"

  # refer to own block
  ref_to_own_value = test.value

  # refer to another data block
  ref_to_others_value = example.value

  attribute_with_object_value = {
    value = "object value"

    # can use references to own block here as well
    abs_ref = test.value

    # not permitted (references into object that is being evaluated)
    #ref_to_abs_ref_into_object = test.attribute_with_object_value.value
  }

  # reference into an object
  ref_into_object = test.attribute_with_object_value.value
}

data example {
  value = "example value"
}
