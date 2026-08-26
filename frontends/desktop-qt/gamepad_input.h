#pragma once

#include <sly/sly.hpp>

struct SDL_Gamepad;

class GamepadInput final {
public:
    struct State {
        sly::ButtonState buttons{};
        bool quick_save = false;
        bool quick_load = false;
    };

    GamepadInput();
    ~GamepadInput();

    GamepadInput(const GamepadInput&) = delete;
    GamepadInput& operator=(const GamepadInput&) = delete;

    [[nodiscard]] State poll();

private:
    SDL_Gamepad* gamepad_ = nullptr;
    bool initialized_ = false;
    bool left_stick_held_ = false;
    bool right_stick_held_ = false;
};
