package com.rev4n.unlimitedphotos;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Answers Google Photos' Pixel feature queries as configured in fgp.prop, purely per-process via
 * a {@link java.lang.reflect.Proxy} over IPackageManager.
 */
final class CustomPackageManager implements InvocationHandler {

    private final Object original;

    /**
     * Features whose configured value was not a boolean. One log line is
     * enough; repeating it on every hasSystemFeature() call flooded logcat.
     */
    private static final Set<String> warned = ConcurrentHashMap.newKeySet();

    CustomPackageManager(Object original) {
        this.original = original;
    }

    @Override
    public Object invoke(Object proxy, Method method, Object[] args) throws Throwable {
        if (args != null && args.length > 0 && args[0] instanceof String
                && "hasSystemFeature".equals(method.getName())) {
            String feature = (String) args[0];
            String override = EntryPoint.getFeatureOverride(feature);
            if (override != null) {
                // Answer from the config before touching the real
                // PackageManager. The binder call is unnecessary when the
                // answer is already known, and it may throw before the
                // override could be returned.
                boolean desired = Boolean.parseBoolean(override);
                if (!"true".equalsIgnoreCase(override) && !"false".equalsIgnoreCase(override)) {
                    if (warned.add(feature)) {
                        EntryPoint.LOG(String.format("[%s]: fgp.prop value '%s' isn't 'true' or 'false', treating as false", feature, override));
                    }
                } else if (EntryPoint.getVerboseLogs() > 1) {
                    EntryPoint.LOG(String.format("[%s]: -> %b (fgp.prop)", feature, desired));
                }
                return desired;
            }
            Object result = invokeOriginal(method, args);
            if (EntryPoint.getVerboseLogs() > 1) {
                EntryPoint.LOG(String.format("hasSystemFeature('%s'): passthrough -> %s", feature, result));
            }
            return result;
        }
        return invokeOriginal(method, args);
    }

    private Object invokeOriginal(Method method, Object[] args) throws Throwable {
        try {
            return method.invoke(original, args);
        } catch (InvocationTargetException e) {
            // Unwrap, or the RemoteException the real PackageManager declares would reach the
            // caller as an UndeclaredThrowableException instead
            throw e.getCause() != null ? e.getCause() : e;
        }
    }
}
