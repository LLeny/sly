#include "gamepad_input.h"

#if SLY_HAS_SDL3
#include <print>

#include <SDL3/SDL_gamepad.h>
#include <SDL3/SDL_hints.h>
#include <SDL3/SDL_init.h>
#include <SDL3/SDL_stdinc.h>
#endif

GamepadInput::GamepadInput() {
#if SLY_HAS_SDL3
    SDL_SetHint(SDL_HINT_HIDAPI_UDEV, "0");

    initialized_ = SDL_InitSubSystem(SDL_INIT_GAMEPAD);
    if (!initialized_) {
        std::println(stderr, "Gamepad support disabled: {}", SDL_GetError());
        return;
    }

    SDL_SetGamepadEventsEnabled(false);
    SDL_UpdateGamepads();

    int count = 0;
    SDL_JoystickID* gamepads = SDL_GetGamepads(&count);
    if (gamepads != nullptr && count > 0) {
        gamepad_ = SDL_OpenGamepad(gamepads[0]);
        if (gamepad_ != nullptr) {
            const char* name = SDL_GetGamepadName(gamepad_);
            std::println("Gamepad connected: {}", name != nullptr ? name : "Unknown gamepad");
        }
    }
    SDL_free(gamepads);
#endif
}

GamepadInput::~GamepadInput() {
#if SLY_HAS_SDL3
    if (gamepad_ != nullptr) {
        SDL_CloseGamepad(gamepad_);
    }
    if (initialized_) {
        SDL_QuitSubSystem(SDL_INIT_GAMEPAD);
    }
#endif
}

GamepadInput::State GamepadInput::poll() {
    State result{};

#if SLY_HAS_SDL3
    if (gamepad_ == nullptr) {
        return result;
    }

    SDL_UpdateGamepads();

    if (!SDL_GamepadConnected(gamepad_)) {
        std::println("Gamepad disconnected");
        SDL_CloseGamepad(gamepad_);
        gamepad_ = nullptr;
        left_stick_held_ = false;
        right_stick_held_ = false;
        return result;
    }

    constexpr Sint16 AXIS_DEAD_ZONE = 16'384;
    const auto button = [this](const SDL_GamepadButton value) {
        return SDL_GetGamepadButton(gamepad_, value);
    };
    const auto axis_low = [this](const SDL_GamepadAxis value) {
        return SDL_GetGamepadAxis(gamepad_, value) < -AXIS_DEAD_ZONE;
    };
    const auto axis_high = [this](const SDL_GamepadAxis value) {
        return SDL_GetGamepadAxis(gamepad_, value) > AXIS_DEAD_ZONE;
    };

    using namespace sly;

    auto& state = result.buttons;
    state[static_cast<std::size_t>(Button::Up)] =
        button(SDL_GAMEPAD_BUTTON_DPAD_UP) || axis_low(SDL_GAMEPAD_AXIS_LEFTY);
    state[static_cast<std::size_t>(Button::Down)] =
        button(SDL_GAMEPAD_BUTTON_DPAD_DOWN) || axis_high(SDL_GAMEPAD_AXIS_LEFTY);
    state[static_cast<std::size_t>(Button::Left)] =
        button(SDL_GAMEPAD_BUTTON_DPAD_LEFT) || axis_low(SDL_GAMEPAD_AXIS_LEFTX);
    state[static_cast<std::size_t>(Button::Right)] =
        button(SDL_GAMEPAD_BUTTON_DPAD_RIGHT) || axis_high(SDL_GAMEPAD_AXIS_LEFTX);
    state[static_cast<std::size_t>(Button::B)] = button(SDL_GAMEPAD_BUTTON_SOUTH);
    state[static_cast<std::size_t>(Button::A)] = button(SDL_GAMEPAD_BUTTON_EAST);
    state[static_cast<std::size_t>(Button::Y)] = button(SDL_GAMEPAD_BUTTON_WEST);
    state[static_cast<std::size_t>(Button::X)] = button(SDL_GAMEPAD_BUTTON_NORTH);
    state[static_cast<std::size_t>(Button::L)] =
        button(SDL_GAMEPAD_BUTTON_LEFT_SHOULDER) || axis_high(SDL_GAMEPAD_AXIS_LEFT_TRIGGER);
    state[static_cast<std::size_t>(Button::R)] =
        button(SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER) || axis_high(SDL_GAMEPAD_AXIS_RIGHT_TRIGGER);
    state[static_cast<std::size_t>(Button::Start)] = button(SDL_GAMEPAD_BUTTON_START);
    state[static_cast<std::size_t>(Button::Select)] = button(SDL_GAMEPAD_BUTTON_BACK);

    const bool right_stick = button(SDL_GAMEPAD_BUTTON_RIGHT_STICK);
    result.quick_save = right_stick && !right_stick_held_;
    right_stick_held_ = right_stick;

    const bool left_stick = button(SDL_GAMEPAD_BUTTON_LEFT_STICK);
    result.quick_load = left_stick && !left_stick_held_;
    left_stick_held_ = left_stick;
#endif

    return result;
}
