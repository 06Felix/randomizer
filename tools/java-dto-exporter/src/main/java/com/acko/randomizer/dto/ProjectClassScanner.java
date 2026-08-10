package com.acko.randomizer.dto;

import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.Field;
import java.lang.reflect.GenericArrayType;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.net.URISyntaxException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;

final class ProjectClassScanner {
    record Result(List<String> classes, String inputHash) {}

    private final Path projectClasses;
    private final ClassLoader classLoader;

    ProjectClassScanner(Path projectClasses, ClassLoader classLoader) {
        this.projectClasses = projectClasses.toAbsolutePath().normalize();
        this.classLoader = classLoader;
    }

    Result scan(Type rootType) {
        Set<Class<?>> visited = new HashSet<>();
        ArrayDeque<Type> pending = new ArrayDeque<>();
        pending.add(rootType);

        while (!pending.isEmpty()) {
            collect(pending.removeFirst(), visited, pending);
        }

        List<Class<?>> projectTypes = visited.stream()
                .filter(this::isProjectClass)
                .sorted(Comparator.comparing(Class::getName))
                .toList();
        return new Result(
                projectTypes.stream().map(Class::getName).toList(),
                "sha256:" + hash(projectTypes)
        );
    }

    private void collect(Type type, Set<Class<?>> visited, ArrayDeque<Type> pending) {
        if (type instanceof ParameterizedType parameterized) {
            pending.add(parameterized.getRawType());
            for (Type argument : parameterized.getActualTypeArguments()) {
                pending.add(argument);
            }
            return;
        }
        if (type instanceof GenericArrayType array) {
            pending.add(array.getGenericComponentType());
            return;
        }
        if (!(type instanceof Class<?> typeClass)) {
            return;
        }
        if (typeClass.isArray()) {
            pending.add(typeClass.getComponentType());
            return;
        }
        if (!visited.add(typeClass) || !isProjectClass(typeClass)) {
            return;
        }

        Type superclass = typeClass.getGenericSuperclass();
        if (superclass != null) {
            pending.add(superclass);
        }
        for (Type interfaceType : typeClass.getGenericInterfaces()) {
            pending.add(interfaceType);
        }
        for (Field field : typeClass.getDeclaredFields()) {
            if (!field.isSynthetic()) {
                pending.add(field.getGenericType());
            }
        }
        for (Method method : typeClass.getDeclaredMethods()) {
            if (isGetter(method)) {
                pending.add(method.getGenericReturnType());
            }
        }
        if (typeClass.isRecord()) {
            for (RecordComponent component : typeClass.getRecordComponents()) {
                pending.add(component.getGenericType());
            }
        }
    }

    private static boolean isGetter(Method method) {
        if (method.isSynthetic() || Modifier.isStatic(method.getModifiers())
                || method.getParameterCount() != 0 || method.getReturnType() == void.class) {
            return false;
        }
        String name = method.getName();
        return (name.startsWith("get") && name.length() > 3)
                || (name.startsWith("is") && name.length() > 2
                && (method.getReturnType() == boolean.class || method.getReturnType() == Boolean.class));
    }

    private boolean isProjectClass(Class<?> type) {
        if (type.isPrimitive() || type.getProtectionDomain() == null
                || type.getProtectionDomain().getCodeSource() == null) {
            return false;
        }
        try {
            Path location = Path.of(type.getProtectionDomain().getCodeSource().getLocation().toURI())
                    .toAbsolutePath()
                    .normalize();
            return Files.isDirectory(location) && location.equals(projectClasses);
        } catch (URISyntaxException | IllegalArgumentException error) {
            return false;
        }
    }

    private String hash(List<Class<?>> classes) {
        try {
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            for (Class<?> type : classes) {
                digest.update(type.getName().getBytes(StandardCharsets.UTF_8));
                digest.update((byte) 0);
                String resource = type.getName().replace('.', '/') + ".class";
                try (InputStream stream = classLoader.getResourceAsStream(resource)) {
                    if (stream == null) {
                        throw new IllegalArgumentException("Compiled bytecode not found for " + type.getName());
                    }
                    digest.update(stream.readAllBytes());
                }
            }
            return HexFormat.of().formatHex(digest.digest());
        } catch (NoSuchAlgorithmException error) {
            throw new IllegalStateException("SHA-256 is not available", error);
        } catch (IOException error) {
            throw new IllegalArgumentException("Failed to read compiled DTO bytecode", error);
        }
    }
}
