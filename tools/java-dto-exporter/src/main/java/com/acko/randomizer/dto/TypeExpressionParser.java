package com.acko.randomizer.dto;

import java.lang.reflect.Array;
import java.lang.reflect.GenericArrayType;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

final class TypeExpressionParser {
    private static final Map<String, Class<?>> PRIMITIVES = Map.of(
            "boolean", boolean.class,
            "byte", byte.class,
            "short", short.class,
            "int", int.class,
            "long", long.class,
            "float", float.class,
            "double", double.class,
            "char", char.class
    );

    private final String input;
    private final ClassLoader classLoader;
    private int offset;

    TypeExpressionParser(String input, ClassLoader classLoader) {
        this.input = input;
        this.classLoader = classLoader;
    }

    Type parse() {
        Type type = parseType();
        skipWhitespace();
        if (offset != input.length()) {
            throw syntax("unexpected trailing input");
        }
        return type;
    }

    private Type parseType() {
        skipWhitespace();
        String name = parseName();
        Class<?> rawType = loadClass(name);
        skipWhitespace();

        Type result = rawType;
        if (consume('<')) {
            List<Type> arguments = new ArrayList<>();
            do {
                arguments.add(parseType());
                skipWhitespace();
            } while (consume(','));
            expect('>');
            if (rawType.getTypeParameters().length != arguments.size()) {
                throw syntax("type " + name + " expects " + rawType.getTypeParameters().length
                        + " generic arguments but received " + arguments.size());
            }
            result = new ParameterizedTypeValue(rawType, arguments.toArray(Type[]::new));
        } else if (rawType.getTypeParameters().length > 0) {
            throw syntax("generic type " + name + " requires " + rawType.getTypeParameters().length
                    + " type arguments");
        }

        while (true) {
            skipWhitespace();
            if (!consume('[')) {
                return result;
            }
            expect(']');
            result = arrayType(result);
        }
    }

    private String parseName() {
        int start = offset;
        while (offset < input.length()) {
            char value = input.charAt(offset);
            if (Character.isJavaIdentifierPart(value) || value == '.' || value == '$') {
                offset++;
            } else {
                break;
            }
        }
        if (start == offset) {
            throw syntax("expected a fully-qualified Java type");
        }
        return input.substring(start, offset);
    }

    private Class<?> loadClass(String name) {
        Class<?> primitive = PRIMITIVES.get(name);
        if (primitive != null) {
            return primitive;
        }
        try {
            return Class.forName(name, false, classLoader);
        } catch (ClassNotFoundException error) {
            throw new IllegalArgumentException("Java type not found on the project classpath: " + name, error);
        }
    }

    private static Type arrayType(Type component) {
        if (component instanceof Class<?> componentClass) {
            return Array.newInstance(componentClass, 0).getClass();
        }
        return new GenericArrayTypeValue(component);
    }

    private boolean consume(char expected) {
        skipWhitespace();
        if (offset < input.length() && input.charAt(offset) == expected) {
            offset++;
            return true;
        }
        return false;
    }

    private void expect(char expected) {
        if (!consume(expected)) {
            throw syntax("expected '" + expected + "'");
        }
    }

    private void skipWhitespace() {
        while (offset < input.length() && Character.isWhitespace(input.charAt(offset))) {
            offset++;
        }
    }

    private IllegalArgumentException syntax(String message) {
        return new IllegalArgumentException(message + " at offset " + offset + " in " + input);
    }

    private record ParameterizedTypeValue(Class<?> rawType, Type[] arguments) implements ParameterizedType {
        @Override
        public Type[] getActualTypeArguments() {
            return arguments.clone();
        }

        @Override
        public Type getRawType() {
            return rawType;
        }

        @Override
        public Type getOwnerType() {
            return rawType.getDeclaringClass();
        }
    }

    private record GenericArrayTypeValue(Type component) implements GenericArrayType {
        @Override
        public Type getGenericComponentType() {
            return component;
        }
    }
}
