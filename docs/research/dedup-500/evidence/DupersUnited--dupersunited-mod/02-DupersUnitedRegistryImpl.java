package wtf.dupers.dupersunited;

import wtf.dupers.dupersunited.api.DupersUnitedRegistry;
import wtf.dupers.dupersunited.api.command.Command;
import wtf.dupers.dupersunited.api.keybind.Keybind;
import wtf.dupers.dupersunited.api.module.Module;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class DupersUnitedRegistryImpl implements DupersUnitedRegistry {
    public final Map<String, Command> commands = new HashMap<>();
    public final List<Module> modules = new ArrayList<>();
    public final List<Keybind> keybinds = new ArrayList<>();
    public String namespace;

    @Override
    public void registerCommand(Command command) {
        String qualified = this.commands.containsKey(command.command)
            ? this.namespace + ":" + command.command
            : command.command;

        command.qualifiedName = qualified;

        if (this.commands.putIfAbsent(qualified, command) != null) {
            MainClient.LOGGER.warn("Registered duplicate subcommand at {}", qualified);
        }
    }

    @Override
    public void registerModule(Module module) {
        module.namespace = namespace;
        this.modules.add(module);
    }

    @Override
    public void registerKeybind(Keybind keybind) {
        this.keybinds.add(keybind);
    }
}
