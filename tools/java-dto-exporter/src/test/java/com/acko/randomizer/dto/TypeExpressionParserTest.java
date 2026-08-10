package com.acko.randomizer.dto;

import org.junit.jupiter.api.Test;

import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class TypeExpressionParserTest {
    @Test
    void preservesNestedGenericArguments() {
        Type parsed = new TypeExpressionParser(
                "java.util.Map<java.lang.String, java.util.List<java.lang.Long>>",
                getClass().getClassLoader()
        ).parse();

        ParameterizedType map = (ParameterizedType) parsed;
        assertEquals(Map.class, map.getRawType());
        assertEquals(String.class, map.getActualTypeArguments()[0]);
        ParameterizedType list = (ParameterizedType) map.getActualTypeArguments()[1];
        assertEquals(List.class, list.getRawType());
        assertEquals(Long.class, list.getActualTypeArguments()[0]);
    }

    @Test
    void rejectsUnboundGenericTypes() {
        assertThrows(IllegalArgumentException.class, () -> new TypeExpressionParser(
                "java.util.List",
                getClass().getClassLoader()
        ).parse());
    }
}
