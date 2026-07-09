<?php

namespace App\Services;

final class GreetingService
{
    public function greet(string $name): string
    {
        return "Hello, {$name}";
    }
}
