<?php

use App\Services\GreetingService;
use PHPUnit\Framework\TestCase;

final class GreetingServiceTest extends TestCase
{
    public function test_greet_returns_message(): void
    {
        self::assertSame('Hello, Ada', (new GreetingService())->greet('Ada'));
    }
}
