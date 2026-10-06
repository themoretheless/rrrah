package wtf.dupers.dupersunited.api;

import wtf.dupers.dupersunited.MainClient;
import wtf.dupers.dupersunited.api.command.Command;
import wtf.dupers.dupersunited.api.module.Module;

import java.util.Collection;
import java.util.Optional;

/**
 * Static functions to interface with the DupersUnited mod API.
 */
public interface DupersUnitedMod {
    /**
     * @return An immutable collection containing all registered {@link Module}.
     */
    static Collection<Module> getModules() {
        return MainClient.getModules();
    }

    /**
     * @return An {@link Optional} containing the specified {@link Module}.
     */
    static <T extends Module> Optional<T> getModule(Class<T> moduleClass) {
        return Optional.ofNullable(MainClient.getModule(moduleClass));
    }

    /**
     * @param moduleName the name or identifier of a {@link Module}.
     * @return An {@link Optional} containing the module that matches the identifier, or first module that matches the name.
     * @see Module#getName()
     * @see Module#getIdentifier()
     */
    static Optional<Module> getModule(String moduleName) {
        return Optional.ofNullable(MainClient.getModule(moduleName));
    }

    /**
     * @return An immutable collection containing all registered {@link Command}.
     */
    static Collection<Command> getCommands() {
        return MainClient.getCommands();
    }

    /**
     * @return An {@link Optional} containing the specified {@link Command}.
     */
    static <T extends Command> Optional<T> getCommand(Class<T> commandClass) {
        return Optional.ofNullable(MainClient.getCommand(commandClass));
    }
}
