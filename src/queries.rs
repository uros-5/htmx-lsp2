pub static HX_NAME: &str = r#"
(
        [
            (_ 
                (tag_name) 

                (_)*

                (attribute (attribute_name) @attr_name) @complete_match

                (#eq? @attr_name @complete_match)
            )

            (_ 
              (tag_name) 

              (attribute (attribute_name)) 

             (ERROR)? @equal_error
            ) @unfinished_tag
        ]

        (#match? @attr_name "hx-.*")
)
    
"#;

pub static HX_VALUE: &str = r#"
(
        [
          (ERROR 
            (tag_name) 

            (attribute_name) @attr_name 
            (_)
          ) @open_quote_error

          (_ 
            (tag_name)

            (attribute 
              (attribute_name) @attr_name
              (_)
            ) @last_item

            (ERROR) @error_char
          )

          (_
            (tag_name)

            (attribute 
              (attribute_name) @attr_name
              (quoted_attribute_value) @quoted_attr_value

              (#eq? @quoted_attr_value "\"\"")
            ) @empty_attribute
          )

          (_
            (tag_name) 

            (attribute 
              (attribute_name) @attr_name
              (quoted_attribute_value (attribute_value) @attr_value)

              ) @non_empty_attribute 
          )
        ]

        (#match? @attr_name "hx-.*")
)"#;
